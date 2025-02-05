use crate::error::{IrcError, IrcResult};
use crate::server::Server;
use crate::ts6::TS6Message;

impl Server {
    pub(crate) async fn handle_server_pass(&self, msg: TS6Message) -> IrcResult<()> {
        // PASS password TS ts sid
        if msg.params.len() < 4 {
            return Err(IrcError::Protocol("Invalid PASS parameters".into()));
        }

        let password = &msg.params[0];
        let ts_version = &msg.params[2];
        let sid = &msg.params[3];

        // Verify TS version
        if ts_version != "6" {
            return Err(IrcError::Protocol("Unsupported TS version".into()));
        }

        // TODO: Verify password and SID
        Ok(())
    }
}