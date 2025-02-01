use crate::error::{IrcError, IrcResult};
use crate::ts6::TS6Message;
use tracing::{debug, warn};
use super::super::Client;

impl Client {
    pub(crate) async fn handle_join(&mut self, message: TS6Message) -> IrcResult<()> {
        if message.params.is_empty() {
            return Err(IrcError::Protocol("No channel specified".into()));
        }

        let channel_name = &message.params[0];
        debug!("Client {} attempting to join channel {}", self.id, channel_name);

        // Check membership first without holding locks
        let is_first_user = !self.server.check_channel_membership(channel_name, self.id).await;

        // Join channel
        {
            let channel = self.server.get_or_create_channel(channel_name).await;
            let mut channel = channel.write().await;
            channel.add_member(self.id);
        }

        // Send JOIN message
        let join_msg = TS6Message::with_source(
            self.get_prefix(),
            "JOIN".to_string(),
            vec![channel_name.clone()]
        );
        self.server.broadcast_to_channel(channel_name, &join_msg, None).await?;

        // Get member list for NAMES
        let names = {
            let channel = self.server.get_or_create_channel(channel_name).await;
            let channel = channel.read().await;
            let member_ids = channel.get_members().clone(); // Clone to avoid holding lock
            drop(channel);

            // Now get member info without holding channel lock
            let mut names = Vec::new();
            for id in member_ids {
                if let Some(client) = self.server.get_client(id).await {
                    if let Ok(client) = client.try_lock() {
                        if let Some(ref nick) = client.nickname {
                            names.push(nick.clone());
                        }
                    }
                }
            }
            names
        };

        // Send NAMES list
        self.send_numeric(353, &[
            "=",
            channel_name,
            &names.join(" ")
        ]).await?;
        self.send_numeric(366, &[channel_name, "End of /NAMES list."]).await?;

        Ok(())
    }

    pub(crate) async fn handle_part(&mut self, message: TS6Message) -> IrcResult<()> {
        if message.params.is_empty() {
            return Err(IrcError::Protocol("No channel specified".into()));
        }

        let channel_name = &message.params[0];
        let part_message = message.params.get(1)
            .map(|s| s.as_str())
            .unwrap_or("Leaving");

        if let Some(channel) = self.server.get_channel(channel_name).await {
            let channel = channel.read().await;
            if !channel.get_members().contains(&self.id) {
                return Err(IrcError::Protocol("Not on channel".into()));
            }

            // Broadcast PART to channel
            let part_msg = TS6Message::with_source(
                self.get_prefix(),
                "PART".to_string(),
                vec![channel_name.to_string(), part_message.to_string()]
            );
            self.server.broadcast_to_channel(channel_name, &part_msg, None).await?;

            // Remove from channel
            self.server.remove_from_channel(channel_name, self.id).await?;
        } else {
            return Err(IrcError::Protocol("No such channel".into()));
        }

        Ok(())
    }

    pub(crate) async fn handle_topic(&mut self, message: TS6Message) -> IrcResult<()> {
        if message.params.is_empty() {
            return Err(IrcError::Protocol("No channel specified".into()));
        }

        let channel_name = &message.params[0];
        let new_topic = message.params.get(1);

        let channel = self.server.get_or_create_channel(channel_name).await;
        let mut channel = channel.write().await;

        if let Some(topic) = new_topic {
            // Setting new topic
            channel.set_topic(topic.clone(), self.get_mask());
            
            // Broadcast topic change
            let topic_msg = TS6Message::with_source(
                self.get_prefix(),
                "TOPIC".to_string(),
                vec![channel_name.clone(), topic.clone()]
            );
            self.server.broadcast_to_channel(channel_name, &topic_msg, None).await?;
        } else {
            // Querying topic
            let (topic, setter, time) = channel.get_topic_details();
            if let Some(topic) = topic {
                self.send_numeric(332, &[channel_name, &topic]).await?;
                self.send_numeric(333, &[
                    channel_name,
                    &setter.unwrap_or_else(|| "unknown".to_string()),
                    &time.timestamp().to_string()
                ]).await?;
            } else {
                self.send_numeric(331, &[channel_name, "No topic is set"]).await?;
            }
        }
        Ok(())
    }

    pub(crate) async fn handle_names(&mut self, channel: &str, members: &[(String, Vec<char>)]) -> IrcResult<()> {
        // Split names into chunks to avoid message length limits
        for chunk in members.chunks(10) {
            let names: String = chunk.iter()
                .map(|(nick, modes)| {
                    let prefix = if modes.contains(&'o') {
                        "@"
                    } else if modes.contains(&'v') {
                        "+"
                    } else {
                        ""
                    };
                    format!("{}{}", prefix, nick)
                })
                .collect::<Vec<_>>()
                .join(" ");

            self.send_numeric(353, &["=", channel, &names]).await?;
        }
        self.send_numeric(366, &[channel, "End of /NAMES list"]).await?;
        Ok(())
    }
} 