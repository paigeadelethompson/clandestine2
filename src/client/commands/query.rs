use regex::Regex;
use tracing::{debug, warn};

use crate::error::{IrcError, IrcResult};
use crate::ts6::TS6Message;

use super::super::Client;

impl Client {
    pub(crate) async fn handle_whois(&mut self, message: TS6Message) -> IrcResult<()> {
        if message.params.is_empty() {
            return Err(IrcError::Protocol("No nickname given".into()));
        }

        let target = &message.params[0];
        debug!("Processing WHOIS for target: {}", target);

        if let Some(info) = self.server.find_client_info(target).await {
            // RPL_WHOISUSER (311)
            self.send_numeric(311, &[
                target,
                &info.username,
                &info.hostname,
                "*",
                &info.realname
            ]).await?;

            // RPL_ENDOFWHOIS (318)
            self.send_numeric(318, &[target, "End of /WHOIS list"]).await?;
        } else {
            // ERR_NOSUCHNICK (401)
            self.send_numeric(401, &[target, "No such nick/channel"]).await?;
        }

        Ok(())
    }

    pub(crate) async fn handle_who(&mut self, message: TS6Message) -> IrcResult<()> {
        let target = message.params.get(0);
        let flags = message.params.get(1).map(|s| s.as_str()).unwrap_or("");

        match target {
            Some(channel_name) if channel_name.starts_with('#') => {
                // WHO on a channel
                if let Some(channel) = self.server.get_channel(channel_name).await {
                    let channel = channel.read().await;
                    let member_ids = channel.get_members();

                    for &member_id in member_ids {
                        if let Some(member) = self.server.get_client(member_id).await {
                            let member = member.lock().await;
                            if let (Some(nick), Some(user)) = (member.get_nickname(), member.get_username()) {
                                self.send_numeric(352, &[
                                    channel_name,
                                    user,
                                    member.get_hostname(),
                                    &self.server_name,
                                    nick,
                                    "H", // Here
                                    "0", // Hop count
                                    member.get_realname().map_or("", String::as_str),
                                ]).await?;
                            }
                        }
                    }
                }
            }
            Some(mask) => {
                // WHO on a mask
                let clients = self.server.get_all_clients().await;
                for client in clients {
                    let client = client.lock().await;
                    if let Some(nick) = client.get_nickname() {
                        if self.server.mask_match(nick, mask) {
                            if let Some(user) = client.get_username() {
                                self.send_numeric(352, &[
                                    "*",
                                    user,
                                    client.get_hostname(),
                                    &self.server_name,
                                    nick,
                                    "H", // Here
                                    "0", // Hop count
                                    client.get_realname().map_or("", String::as_str),
                                ]).await?;
                            }
                        }
                    }
                }
            }
            None => {
                // WHO with no target - list all visible users
                let clients = self.server.get_all_clients().await;
                for client in clients {
                    let client = client.lock().await;
                    if let (Some(nick), Some(user)) = (client.get_nickname(), client.get_username()) {
                        // Skip invisible users unless operator
                        if client.modes.contains(&'i') && !self.modes.contains(&'o') {
                            continue;
                        }
                        self.send_numeric(352, &[
                            "*",
                            user,
                            client.get_hostname(),
                            &self.server_name,
                            nick,
                            "H", // Here
                            "0", // Hop count
                            client.get_realname().map_or("", String::as_str),
                        ]).await?;
                    }
                }
            }
        }

        // End of WHO list
        self.send_numeric(315, &[target.map_or("*", |v| v), "End of WHO list"]).await?;
        Ok(())
    }

    pub(crate) async fn handle_whowas(&mut self, message: TS6Message) -> IrcResult<()> {
        if message.params.is_empty() {
            return Err(IrcError::Protocol("No nickname given".into()));
        }

        // WHOWAS is not implemented yet since we don't store history
        self.send_numeric(406, &[&message.params[0], "There was no such nickname"]).await?;
        self.send_numeric(369, &[&message.params[0], "End of WHOWAS"]).await?;

        Ok(())
    }

    pub(crate) async fn handle_list(&mut self, message: TS6Message) -> IrcResult<()> {
        // RPL_LISTSTART (321)
        self.send_numeric(321, &["Channel", "Users  Name"]).await?;

        // Get all channels through the public interface
        let channel_list = self.server.get_channel_list().await;
        for (name, user_count, topic) in channel_list {
            // RPL_LIST (322)
            self.send_numeric(322, &[
                &name,
                &user_count.to_string(),
                &topic.as_deref().unwrap_or("")
            ]).await?;
        }

        // RPL_LISTEND (323)
        self.send_numeric(323, &["End of /LIST"]).await?;
        Ok(())
    }

    pub(crate) async fn handle_privmsg(&mut self, message: TS6Message) -> IrcResult<()> {
        // Check if client is registered first
        if !self.registered {
            return Err(IrcError::Protocol("You must register first".into()));
        }

        // Check for required parameters
        let (target, text) = match (message.params.get(0), message.params.get(1)) {
            (Some(target), Some(text)) => (target, text),
            _ => return Err(IrcError::Protocol("Not enough parameters".into())),
        };

        debug!("PRIVMSG from {} to {}: {}", self.get_nickname().unwrap_or(&"*".to_string()), target, text);

        // Handle channel messages
        if target.starts_with('#') {
            // Check if user is in channel
            if !self.server.check_channel_membership(target, self.id).await {
                return self.send_numeric(442, &[target, "You're not on that channel"]).await;
            }

            // Create message with source
            let msg = TS6Message::with_source(
                self.get_prefix(),
                "PRIVMSG".to_string(),
                vec![target.to_string(), text.to_string()],
            );

            // Broadcast to channel (excluding sender)
            self.server.broadcast_to_channel(target, &msg, Some(self.id)).await
        } else {
            // Handle private messages to users
            if let Some(target_client) = self.server.find_client_by_nick(target).await {
                let msg = TS6Message::with_source(
                    self.get_prefix(),
                    "PRIVMSG".to_string(),
                    vec![target.to_string(), text.to_string()],
                );

                let target_client = target_client.lock().await;
                target_client.send_message(&msg).await
            } else {
                self.send_numeric(401, &[target, "No such nick/channel"]).await
            }
        }
    }

    pub(crate) async fn handle_notice(&mut self, message: TS6Message) -> IrcResult<()> {
        // Check if client is registered first
        if !self.registered {
            return Ok(()); // Silently ignore per RFC
        }

        // Check for required parameters
        let (target, text) = match (message.params.get(0), message.params.get(1)) {
            (Some(target), Some(text)) => (target, text),
            _ => return Ok(()),  // Silently ignore per RFC
        };

        debug!("NOTICE from {} to {}: {}", self.get_nickname().unwrap_or(&"*".to_string()), target, text);

        // Handle channel messages
        if target.starts_with('#') {
            // Check if user is in channel - silently ignore if not
            if self.server.check_channel_membership(target, self.id).await {
                // Create message with source
                let msg = TS6Message::with_source(
                    self.get_prefix(),
                    "NOTICE".to_string(),
                    vec![target.to_string(), text.to_string()],
                );

                // Broadcast to channel (excluding sender)
                self.server.broadcast_to_channel(target, &msg, Some(self.id)).await?;
            }
        } else {
            // Handle private notices to users
            if let Some(target_client) = self.server.find_client_by_nick(target).await {
                let msg = TS6Message::with_source(
                    self.get_prefix(),
                    "NOTICE".to_string(),
                    vec![target.to_string(), text.to_string()],
                );

                let target_client = target_client.lock().await;
                target_client.send_message(&msg).await?;
            }
        }

        Ok(())
    }
} 