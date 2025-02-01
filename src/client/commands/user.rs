use crate::error::{IrcError, IrcResult};
use crate::ts6::TS6Message;
use tracing::{debug, info};
use super::super::Client;

impl Client {
    pub(crate) async fn handle_nick(&mut self, message: TS6Message) -> IrcResult<()> {
        // Enforce CAP negotiation completion before NICK
        if self.cap_negotiating {
            return Err(IrcError::Protocol("Must complete capability negotiation first (CAP END)".into()));
        }

        if message.params.is_empty() {
            return Err(IrcError::Protocol("No nickname given".into()));
        }

        let new_nick = &message.params[0];
        debug!("Client {} requesting nick change to {}", self.id, new_nick);

        // Check nickname format
        if !new_nick.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
            return Err(IrcError::Protocol("Invalid nickname".into()));
        }

        // Check if nickname is available
        if !self.server.check_nickname(new_nick).await {
            self.send_numeric(433, &[new_nick, "Nickname is already in use"]).await?;
            return Ok(());
        }

        // If we already had a nickname, unregister it
        if let Some(old_nick) = &self.nickname {
            self.server.unregister_nickname(old_nick).await;
        }

        // Register new nickname
        if let Err(e) = self.server.register_nickname(new_nick, self.id).await {
            self.send_numeric(433, &[new_nick, "Nickname is already in use"]).await?;
            return Ok(());
        }

        // Update nickname
        let old_nick = self.nickname.clone();
        self.nickname = Some(new_nick.to_string());

        // If this is a nick change (not initial registration)
        if let Some(old_nick) = old_nick {
            let nick_msg = TS6Message::with_source(
                self.get_prefix(),
                "NICK".to_string(),
                vec![new_nick.to_string()]
            );
            self.server.broadcast_global(&nick_msg.to_string()).await?;
        }

        // Try registration if we have both nick and user
        self.check_registration().await?;

        Ok(())
    }

    pub(crate) async fn handle_user(&mut self, message: TS6Message) -> IrcResult<()> {
        // Enforce CAP negotiation completion before USER
        if self.cap_negotiating {
            return Err(IrcError::Protocol("Must complete capability negotiation first (CAP END)".into()));
        }

        // Check if already registered
        if self.registered {
            return Err(IrcError::Protocol("Already registered".into()));
        }

        // Check for required parameters
        if message.params.len() < 4 {
            return Err(IrcError::Protocol("Not enough parameters".into()));
        }

        let username = &message.params[0];
        let realname = &message.params[3];

        // Store user information
        self.username = Some(username.clone());
        self.realname = Some(realname.clone());

        debug!("Client {} attempting registration after USER", self.id);
        self.check_registration().await?;

        Ok(())
    }

    pub(crate) async fn handle_mode(&mut self, message: TS6Message) -> IrcResult<()> {
        if message.params.is_empty() {
            return Err(IrcError::Protocol("Not enough parameters".into()));
        }

        let target = &message.params[0];
        
        // User modes
        if let Some(ref nick) = self.nickname {
            if target == nick {
                if message.params.len() == 1 {
                    // Query user modes
                    let modes: String = self.modes.iter().collect();
                    self.send_numeric(221, &[&format!("+{}", modes)]).await?;
                    return Ok(());
                }

                let modes = &message.params[1];
                let mut adding = true;

                for c in modes.chars() {
                    match c {
                        '+' => adding = true,
                        '-' => adding = false,
                        'i' | 'w' | 'o' | 'O' | 'r' => {
                            if adding {
                                self.modes.insert(c);
                            } else {
                                self.modes.remove(&c);
                            }
                        }
                        _ => continue,
                    }
                }
                return Ok(());
            }
        }

        // Channel modes handled elsewhere
        if target.starts_with('#') {
            return Err(IrcError::Protocol("Channel modes not implemented".into()));
        }

        Err(IrcError::Protocol("Invalid mode target".into()))
    }

    pub(crate) async fn handle_quit(&mut self, message: TS6Message) -> IrcResult<()> {
        let quit_message = message.params.first()
            .map(|s| s.as_str())
            .unwrap_or("Client Quit");

        let quit_msg = TS6Message::with_source(
            self.get_prefix(),
            "QUIT".to_string(),
            vec![quit_message.to_string()]
        );

        // Broadcast quit to all channels user is in
        self.server.broadcast_global(&quit_msg.to_string()).await?;

        // The actual cleanup is handled by the connection handler
        Ok(())
    }
} 