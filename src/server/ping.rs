use crate::error::{IrcError, IrcResult};
use crate::server::Server;
use crate::ts6::TS6Message;

impl Server {
    pub(crate) async fn handle_server_ping(&self, msg: TS6Message) -> IrcResult<()> {
        // PING source [destination]
        if msg.params.is_empty() {
            return Err(IrcError::Protocol("No PING source".into()));
        }

        // Send PONG response
        let pong = TS6Message::new(
            "PONG".to_string(),
            vec![self.config.server.name.clone(), msg.params[0].clone()],
        );

        // TODO: Send to correct server
        Ok(())
    }

    pub(crate) async fn handle_server_pong(&self, _msg: TS6Message) -> IrcResult<()> {
        Ok(())
    }
}
