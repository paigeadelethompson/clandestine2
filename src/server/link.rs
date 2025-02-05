use tracing::info;

use crate::error::{IrcError, IrcResult};
use crate::server::Server;
use crate::ts6::TS6Message;

impl Server {
    pub(crate) async fn handle_server_intro(&self, msg: TS6Message) -> IrcResult<()> {
        // SERVER name hopcount description
        if msg.params.len() < 3 {
            return Err(IrcError::Protocol("Invalid SERVER parameters".into()));
        }

        let name = &msg.params[0];
        let description = &msg.params[2];

        // TODO: Add server to network topology
        info!("Server {} introduced: {}", name, description);
        Ok(())
    }

    pub(crate) async fn handle_server_join(&self, msg: TS6Message) -> IrcResult<()> {
        // SJOIN timestamp channel modes members
        if msg.params.len() < 4 {
            return Err(IrcError::Protocol("Invalid SJOIN parameters".into()));
        }

        // TODO: Process channel join with TS
        Ok(())
    }

    pub(crate) async fn handle_server_sid(&self, msg: TS6Message) -> IrcResult<()> {
        // SID name hopcount sid description
        if msg.params.len() < 4 {
            return Err(IrcError::Protocol("Invalid SID parameters".into()));
        }

        // TODO: Process server introduction
        Ok(())
    }

    pub(crate) async fn handle_server_quit(&self, msg: TS6Message) -> IrcResult<()> {
        // SQUIT server reason
        if msg.params.len() < 2 {
            return Err(IrcError::Protocol("Invalid SQUIT parameters".into()));
        }

        let server = &msg.params[0];
        let reason = &msg.params[1];

        info!("Server {} quit: {}", server, reason);
        // TODO: Remove server and its users from network
        Ok(())
    }
}