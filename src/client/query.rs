use regex::Regex;
use tracing::{debug, warn};

use crate::client::Client;
use crate::error::{IrcError, IrcResult};
use crate::ts6::TS6Message;

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
        if message.params.is_empty() {
            return Err(IrcError::Protocol("No WHO target".into()));
        }

        let target = &message.params[0];

        if target.starts_with('#') {
            // Channel WHO
            let channel = self.server.get_channel(target).await
                .ok_or_else(|| IrcError::Protocol("No such channel".into()))?;
            let channel = channel.read().await;

            // Send WHO reply for each member
            for &member_id in channel.get_members() {
                if let Some(member) = self.server.get_client(member_id).await {
                    let member = member.lock().await;
                    let nick = member.get_nickname().unwrap();
                    let user = member.get_username().unwrap();
                    let host = member.get_hostname();
                    let modes = if channel.has_mode('o', Some(nick)) { "@" } else { "" };

                    // RPL_WHOREPLY
                    self.send_numeric(352, &[
                        target,
                        user,
                        host,
                        self.server_name.as_str(),
                        nick,
                        &format!("H{}", modes),
                        "0",
                        &member.get_realname().map_or_else(String::new, |s| s.to_string()),
                    ]).await?;
                }
            }
        } else {
            // User WHO
            if let Some(client) = self.server.find_client_by_nick(target).await {
                let client = client.lock().await;
                let nick = client.get_nickname().unwrap();
                let user = client.get_username().unwrap();
                let host = client.get_hostname();

                self.send_numeric(352, &[
                    "*",
                    user,
                    host,
                    self.server_name.as_str(),
                    nick,
                    "H",
                    "0",
                    &client.get_realname().map_or_else(String::new, |s| s.to_string()),
                ]).await?;
            }
        }

        // RPL_ENDOFWHO
        self.send_numeric(315, &[target, "End of WHO list"]).await?;

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