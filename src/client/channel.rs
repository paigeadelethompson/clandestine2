use tracing::{debug, warn};

use crate::client::Client;
use crate::error::{IrcError, IrcResult};
use crate::ts6::TS6Message;

impl Client {
    pub(crate) async fn handle_join(&mut self, message: TS6Message) -> IrcResult<()> {
        if message.params.is_empty() {
            return Err(IrcError::Protocol("No channel specified".into()));
        }

        let channel_name = &message.params[0];
        debug!("Client {} attempting to join channel {}", self.id, channel_name);

        // Get channel info with minimal lock time
        let (is_first, topic_info, member_list) = {
            let channel = self.server.get_or_create_channel(channel_name).await;
            let mut channel = channel.write().await;

            let is_first = channel.get_members().is_empty();
            channel.add_member(self.id);

            // Capture needed info while we have the lock
            let topic_info = if let Some(topic) = channel.get_topic() {
                let (_, setter, time) = channel.get_topic_details();
                Some((topic, setter, time))
            } else {
                None
            };

            let member_list = channel.get_members().iter().cloned().collect::<Vec<_>>();

            if is_first {
                channel.set_mode('o', Some(self.get_nickname().unwrap().to_string()), true);
            }

            (is_first, topic_info, member_list)
        };

        // Send JOIN confirmation
        let join_msg = TS6Message::with_source(
            self.get_prefix(),
            "JOIN".to_string(),
            vec![channel_name.to_string()],
        );

        // First send directly to joining client
        self.send_message(&join_msg).await?;

        // Then broadcast to ALL other members (no skip)
        self.server.broadcast_to_channel(channel_name, &join_msg, None).await?;

        // Send operator status if first user
        if is_first {
            let mode_msg = TS6Message::new(
                "MODE".to_string(),
                vec![
                    channel_name.to_string(),
                    "+o".to_string(),
                    self.get_nickname().unwrap().to_string(),
                ],
            );
            self.send_message(&mode_msg).await?;
            self.server.broadcast_to_channel(channel_name, &mode_msg, Some(self.id)).await?;
        }

        // Send topic
        if let Some((topic, setter, time)) = topic_info {
            self.send_numeric(332, &[channel_name, &topic]).await?;
            if let Some(setter) = setter {
                self.send_numeric(333, &[channel_name, &setter, &time.timestamp().to_string()]).await?;
            }
        } else {
            self.send_numeric(331, &[channel_name, "No topic is set"]).await?;
        }

        // Get member info without channel lock
        let mut members = Vec::new();
        for id in member_list {
            if let Some(client) = self.server.get_client(id).await {
                let client = client.lock().await;
                let nick = client.get_nickname().unwrap().to_string();
                let modes = if is_first && id == self.id { vec!['o'] } else { Vec::new() };
                members.push((nick, modes));
            }
        }

        // Send NAMES list
        self.handle_names(channel_name, &members).await?;

        Ok(())
    }

    pub(crate) async fn handle_part(&mut self, message: TS6Message) -> IrcResult<()> {
        if message.params.is_empty() {
            return Err(IrcError::Protocol("No channel given".into()));
        }

        let channel_name = &message.params[0];
        let part_message = message.params.get(1)
            .map(|s| s.as_str())
            .unwrap_or("Leaving");

        debug!("Client {} attempting to part channel {}", self.id, channel_name);

        // Check if client is in the channel
        if !self.server.check_channel_membership(channel_name, self.id).await {
            return Err(IrcError::Protocol("Not on channel".into()));
        }

        // Send PART message to channel
        let part_msg = TS6Message::with_source(
            self.get_prefix(),
            "PART".to_string(),
            vec![channel_name.to_string(), part_message.to_string()],
        );
        self.server.broadcast_to_channel(channel_name, &part_msg, Some(self.id)).await?;

        // Remove client from channel
        self.server.remove_from_channel(channel_name, self.id).await?;

        // Send PART message to parting client
        self.send_message(&part_msg).await?;

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

    pub(crate) async fn handle_channel_mode(&mut self, message: TS6Message) -> IrcResult<()> {
        if message.params.is_empty() {
            return Err(IrcError::Protocol("Not enough parameters".into()));
        }

        let target = &message.params[0];

        // Channel modes
        if target.starts_with('#') {
            let channel = self.server.get_channel(target).await
                .ok_or_else(|| IrcError::Protocol("No such channel".into()))?;

            debug!("Got channel for mode change: {}", target);
            if message.params.len() == 1 {
                // Query channel modes
                let channel = channel.read().await;
                let modes = channel.get_modes();
                self.send_numeric(324, &[target, &format!("+{}", modes)]).await?;
                return Ok(());
            }

            let modes = &message.params[1];
            let mut mode_params = message.params.iter().skip(2);
            let mut adding = true;
            let mut changes = Vec::new();

            let mut channel = channel.write().await;

            for c in modes.chars() {
                match c {
                    '+' => adding = true,
                    '-' => adding = false,
                    'n' | 't' | 'm' | 'i' | 's' => {
                        channel.set_mode(c, None, adding);
                        changes.push((c, None, adding));
                    }
                    'k' => {
                        if adding {
                            if let Some(key) = mode_params.next() {
                                channel.set_mode(c, Some(key.to_string()), true);
                                changes.push((c, Some(key), true));
                            }
                        } else {
                            channel.set_mode(c, None, false);
                            changes.push((c, None, false));
                        }
                    }
                    _ => continue,
                }
            }

            // Broadcast mode changes
            if !changes.is_empty() {
                let mut mode_str = String::new();
                let mut params = Vec::new();

                // Always start with + or - based on first change
                mode_str.push(if changes[0].2 { '+' } else { '-' });

                let mut current = changes[0].2;
                for (mode, param, is_adding) in changes {
                    if is_adding != current {
                        current = is_adding;
                        mode_str.push(if is_adding { '+' } else { '-' });
                    }
                    mode_str.push(mode);
                    if let Some(param) = param {
                        params.push(param.to_string());
                    }
                }

                let mode_msg = TS6Message::with_source(
                    self.get_prefix(),
                    "MODE".to_string(),
                    vec![target.to_string(), mode_str.clone()]
                        .into_iter()
                        .chain(params.clone())
                        .collect(),
                );

                // Send to channel members
                self.server.broadcast_to_channel(target, &mode_msg, None).await?;

                // Send immediate response back to the client that sent the mode command
                let response = format!(":{} MODE {} {}", self.server_name, target, mode_str);
                self.write_raw(response.as_bytes()).await?;
            }

            Ok(())
        } else {
            // User modes handled elsewhere
            self.handle_user_mode(message).await
        }
    }
} 