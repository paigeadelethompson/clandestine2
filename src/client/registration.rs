use chrono::Local;
use tracing::{debug, info};

use crate::error::IrcResult;

use super::Client;

impl Client {
    pub(crate) async fn check_registration(&mut self) -> IrcResult<()> {
        debug!("Client {} registration check: nick={:?}, user={:?}, registered={}, cap_negotiating={}", 
            self.id, self.nickname, self.username, self.registered, self.cap_negotiating);

        if !self.registered &&
            self.nickname.is_some() &&
            self.username.is_some() &&
            !self.cap_negotiating {
            debug!("All registration requirements met for client {}, completing registration", self.id);
            self.complete_registration().await?;
        } else {
            debug!("Client {} not ready for registration", self.id);
        }
        Ok(())
    }

    pub(crate) async fn complete_registration(&mut self) -> IrcResult<()> {
        info!("Client {} registered as {}", self.id, self.nickname.as_ref().unwrap());
        debug!("Sending registration messages to client {}", self.id);

        self.registered = true;

        // Send welcome messages
        self.send_numeric(001, &[&format!("Welcome to {} {}", self.server_name, self.get_mask())]).await?;
        self.send_numeric(002, &[&format!("Your host is {}, running ircd-rs v0.1.0", self.server_name)]).await?;
        self.send_numeric(003, &[&format!("This server was created {}", chrono::Utc::now().format("%Y-%m-%d"))]).await?;
        self.send_numeric(004, &[&self.server_name, "ircd-rs-0.1.0", "iowghraAsORTVSxNCWqBzvdHtGp", "bkloveqjfI", "bklov"]).await?;

        // Send ISUPPORT
        self.send_numeric(005, &["CHANTYPES=# EXCEPTS INVEX CHANMODES=eIbq,k,flj,CFLMPQScgimnprstuz CHANLIMIT=#:100 PREFIX=(ov)@+ MAXLIST=bqeI:100 MODES=4 NETWORK=ExampleNet STATUSMSG=@+ CALLERID=g CASEMAPPING=rfc1459 :are supported by this server"]).await?;

        // Send LUSERS
        self.send_numeric(251, &[&format!("There are {} users and {} invisible on {} server", 0, 0, 1)]).await?;
        self.send_numeric(252, &["0", "operator(s) online"]).await?;
        self.send_numeric(254, &["0", "channels formed"]).await?;
        self.send_numeric(255, &[&format!("I have {} clients and {} servers", 1, 0)]).await?;
        self.send_numeric(265, &[&format!("Current local users: {}, Max: {}", 1, 1)]).await?;
        self.send_numeric(266, &[&format!("Current global users: {}, Max: {}", 1, 1)]).await?;

        // Send MOTD
        self.send_numeric(375, &["- Message of the day"]).await?;
        self.send_numeric(372, &["- Welcome to IRCd-rs!"]).await?;
        self.send_numeric(376, &["End of /MOTD command."]).await?;

        // Start ping timer after registration is complete
        self.start_ping_timer();

        debug!("Completed registration sequence for client {}", self.id);
        Ok(())
    }
} 