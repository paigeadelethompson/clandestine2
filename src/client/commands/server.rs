use tracing::warn;
use super::*;
use crate::error::{IrcError, IrcResult};
use crate::ts6::TS6Message;
use tracing::debug;
use super::super::Client;
use chrono::Local;
use std::time::Instant;

impl Client {
    pub(crate) async fn handle_ping(&mut self, message: TS6Message) -> IrcResult<()> {
        debug!("Received PING from client {}", self.id);
        
        let param = message.params.first()
            .ok_or_else(|| IrcError::Protocol("No ping parameter".into()))?;
            
        debug!("Sending PONG response to client {} with param: {}", self.id, param);
        
        // Send raw PONG response - use exact format client expects
        let pong = format!(":{} PONG {} :{}\r\n", self.server_name, self.server_name, param);
        self.write_raw(pong.as_bytes()).await
    }

    pub(crate) async fn handle_pong(&mut self, message: TS6Message) -> IrcResult<()> {
        debug!("Received PONG from client {}", self.id);
        // Send notification through the pong channel
        if let Err(e) = self.pong_tx.send(()) {
            warn!("Failed to send PONG notification: {}", e);
        }
        Ok(())
    }

    pub(crate) async fn handle_motd(&mut self, _message: TS6Message) -> IrcResult<()> {
        // RPL_MOTDSTART (375)
        self.send_numeric(375, &["- Message of the day"]).await?;
        // RPL_MOTD (372)
        self.send_numeric(372, &["- Welcome to IRCd-rs!"]).await?;
        // RPL_ENDOFMOTD (376)
        self.send_numeric(376, &["End of /MOTD command."]).await?;
        Ok(())
    }

    pub(crate) async fn handle_version(&mut self, _message: TS6Message) -> IrcResult<()> {
        self.send_numeric(351, &[
            "ircd-rs-0.1.0",
            &self.server_name,
            "Available on GitHub"
        ]).await?;
        Ok(())
    }

    pub(crate) async fn handle_time(&mut self, _message: TS6Message) -> IrcResult<()> {
        let time = Local::now();
        self.send_numeric(391, &[&self.server_name, &time.to_rfc2822()]).await?;
        Ok(())
    }

    pub(crate) async fn handle_admin(&mut self, _message: TS6Message) -> IrcResult<()> {
        // RPL_ADMINME (256)
        self.send_numeric(256, &[&self.server_name, "Administrative info"]).await?;
        // RPL_ADMINLOC1 (257)
        self.send_numeric(257, &["Location: Earth"]).await?;
        // RPL_ADMINLOC2 (258)
        self.send_numeric(258, &["Server Info"]).await?;
        // RPL_ADMINEMAIL (259)
        self.send_numeric(259, &["admin@example.com"]).await?;
        Ok(())
    }

    pub(crate) async fn handle_info(&mut self, _message: TS6Message) -> IrcResult<()> {
        // RPL_INFO (371)
        self.send_numeric(371, &["IRCd-rs Server"]).await?;
        self.send_numeric(371, &["Written in Rust"]).await?;
        // RPL_ENDOFINFO (374)
        self.send_numeric(374, &["End of /INFO list"]).await?;
        Ok(())
    }

    pub(crate) async fn handle_lusers(&mut self, _message: TS6Message) -> IrcResult<()> {
        let stats = self.server.get_stats().await;
        
        // RPL_LUSERCLIENT (251)
        self.send_numeric(251, &[&format!(
            "There are {} users and {} invisible on 1 server",
            stats.visible_users,
            stats.invisible_users
        )]).await?;

        // RPL_LUSEROP (252)
        self.send_numeric(252, &[
            &stats.oper_count.to_string(),
            "operator(s) online"
        ]).await?;

        // RPL_LUSERCHANNELS (254)
        self.send_numeric(254, &[
            &stats.channel_count.to_string(),
            "channels formed"
        ]).await?;

        // RPL_LUSERME (255)
        self.send_numeric(255, &[&format!(
            "I have {} clients and {} servers",
            stats.local_users,
            stats.local_servers
        )]).await?;

        Ok(())
    }
} 