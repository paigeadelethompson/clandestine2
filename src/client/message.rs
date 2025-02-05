use tracing::{debug, warn};

use crate::client::Client;
use crate::error::{IrcError, IrcResult};
use crate::ts6::TS6Message;

impl Client {
    pub async fn send_message(&self, message: &TS6Message) -> IrcResult<()> {
        let msg_string = message.to_string();
        debug!("Sending message to client {}: {:?}", self.id, msg_string);

        let mut data = msg_string.into_bytes();
        data.extend_from_slice(b"\r\n");

        // Use sendq for normal messages
        self.sendq_tx.send(data).map_err(|_| {
            let err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "Channel closed");
            IrcError::Io(err)
        })?;

        debug!("Successfully queued message for client {}", self.id);
        Ok(())
    }

    pub(crate) async fn handle_message(&mut self, message: TS6Message) -> IrcResult<()> {
        debug!("Handling message: {:?}", message);

        match message.command.as_str() {
            // CAP must be handled first
            "CAP" => {
                let result = self.handle_cap_command(message).await;
                // Force a flush after CAP command to ensure response is sent
                self.write_raw(b"").await?;
                result
            }

            // During CAP negotiation, buffer commands instead of rejecting
            cmd if self.cap_negotiating => {
                if cmd == "QUIT" {
                    self.handle_quit(message).await
                } else {
                    // Buffer command until CAP END
                    debug!("Buffering command {} during CAP negotiation", cmd);
                    Ok(())
                }
            }

            // Normal command handling
            "NICK" => self.handle_nick(message).await,
            "USER" => self.handle_user(message).await,
            "QUIT" => self.handle_quit(message).await,
            "JOIN" => {
                debug!("Received JOIN command with params: {:?}", message.params);
                self.handle_join(message).await
            }
            "PART" => {
                debug!("Received PART command with params: {:?}", message.params);
                self.handle_part(message).await
            }
            "WHOIS" => {
                debug!("Received WHOIS command with params: {:?}", message.params);
                self.handle_whois(message).await
            }
            "PING" => self.handle_ping(message).await,
            "PONG" => self.handle_pong(message).await,
            "MODE" => {
                let target = message.params.get(0).ok_or_else(||
                IrcError::Protocol("No mode target".into()))?;

                if target.starts_with('#') {
                    self.handle_channel_mode(message).await
                } else {
                    self.handle_user_mode(message).await
                }
            }
            "PRIVMSG" => self.handle_privmsg(message).await,
            "NOTICE" => self.handle_notice(message).await,
            "MOTD" => self.handle_motd(message).await,
            "LUSERS" => self.handle_lusers(message).await,
            "VERSION" => self.handle_version(message).await,
            "ADMIN" => self.handle_admin(message).await,
            "INFO" => self.handle_info(message).await,
            "WHO" => self.handle_who(message).await,
            cmd => {
                warn!("Unknown command from client {}: {}", self.id, cmd);
                self.send_numeric(421, &[&message.command, "Unknown command"]).await
            }
        }
    }
}