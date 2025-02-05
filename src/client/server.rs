use std::time::Instant;

use chrono::Local;
use tracing::debug;
use tracing::warn;

use crate::error::{IrcError, IrcResult};
use crate::ts6::TS6Message;

use super::*;

impl Client {
    pub(crate) async fn handle_ping(&mut self, message: TS6Message) -> IrcResult<()> {
        debug!("Received PING from client {}", self.id);

        // Get the cookie parameter
        let cookie = message.params.first()
            .ok_or_else(|| IrcError::Protocol("No ping parameter".into()))?;

        debug!("Sending PONG response to client {} with cookie: {}", self.id, cookie);

        // Send PONG response with the same cookie value
        let pong = format!(":{} PONG :{}\r\n", self.server_name, cookie);
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