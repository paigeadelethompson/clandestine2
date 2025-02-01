use crate::error::{IrcError, IrcResult};
use crate::ts6::TS6Message;
use tracing::{debug, info, warn};
use super::super::Client;

impl Client {
    pub(crate) async fn handle_oper(&mut self, message: TS6Message) -> IrcResult<()> {
        if message.params.len() < 2 {
            return Err(IrcError::Protocol("Not enough parameters".into()));
        }

        let username = &message.params[0];
        let password = &message.params[1];

        // Check if the user matches any O:lines
        if !self.server.has_oline(self).await {
            warn!("Failed OPER attempt from {}: no matching O:line", self.get_mask());
            self.send_numeric(491, &["No O:lines for your host"]).await?;
            return Ok(());
        }

        // TODO: Implement proper password checking
        // For now, just grant operator status
        self.modes.insert('o');
        info!("Client {} is now an operator", self.id);

        // RPL_YOUREOPER (381)
        self.send_numeric(381, &["You are now an IRC operator"]).await?;

        // Send updated user modes
        let modes: String = self.modes.iter().collect();
        self.send_numeric(221, &[&format!("+{}", modes)]).await?;

        Ok(())
    }

    pub(crate) async fn handle_kill(&mut self, message: TS6Message) -> IrcResult<()> {
        if !self.server.has_oline(self).await {
            return Err(IrcError::Protocol("Permission Denied".into()));
        }
        
        if message.params.len() < 2 {
            return Err(IrcError::Protocol("Not enough parameters".into()));
        }

        let target = &message.params[0];
        let reason = &message.params[1];

        if let Some(client) = self.server.find_client_by_nick(target).await {
            let kill_msg = format!("Killed by {}: {}", self.nickname.as_ref().unwrap(), reason);
            let mut client = client.lock().await;
            client.send_error(&kill_msg).await?;
            // The client will be cleaned up when their connection handler exits
        } else {
            self.send_numeric(401, &[target, "No such nick/channel"]).await?;
        }

        Ok(())
    }

    pub(crate) async fn handle_die(&mut self, _message: TS6Message) -> IrcResult<()> {
        if !self.server.has_oline(self).await {
            return Err(IrcError::Protocol("Permission Denied".into()));
        }

        info!("DIE command received from operator {}", self.get_mask());
        // TODO: Implement graceful shutdown
        std::process::exit(0);
    }

    pub(crate) async fn handle_rehash(&mut self, _message: TS6Message) -> IrcResult<()> {
        if !self.server.has_oline(self).await {
            return Err(IrcError::Protocol("Permission Denied".into()));
        }

        info!("REHASH command received from operator {}", self.get_mask());
        // TODO: Implement config rehash
        self.send_numeric(382, &["ircd.conf", "Rehashing"]).await?;

        Ok(())
    }

    pub(crate) async fn handle_restart(&mut self, _message: TS6Message) -> IrcResult<()> {
        if !self.server.has_oline(self).await {
            return Err(IrcError::Protocol("Permission Denied".into()));
        }

        info!("RESTART command received from operator {}", self.get_mask());
        // TODO: Implement server restart
        self.send_numeric(382, &["ircd", "Restarting"]).await?;
        std::process::exit(0);
    }

    pub(crate) async fn handle_wallops(&mut self, message: TS6Message) -> IrcResult<()> {
        if !self.server.has_oline(self).await {
            return Err(IrcError::Protocol("Permission Denied".into()));
        }

        if message.params.is_empty() {
            return Err(IrcError::Protocol("No message specified".into()));
        }

        let text = &message.params[0];
        let wallops_msg = TS6Message::with_source(
            self.get_prefix(),
            "WALLOPS".to_string(),
            vec![text.clone()]
        );

        // Broadcast to all users with +w mode
        self.server.broadcast_global(&wallops_msg.to_string()).await?;
        Ok(())
    }
} 