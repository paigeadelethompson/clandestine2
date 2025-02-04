use crate::error::{IrcError, IrcResult};
use crate::ts6::TS6Message;
use crate::ircv3::Capability;
use tracing::{debug, warn};
use super::Client;

impl Client {
    pub(crate) async fn handle_cap_command(&mut self, message: TS6Message) -> IrcResult<()> {
        debug!("Handling CAP command: {:?}", message);
        
        if message.params.is_empty() {
            return Err(IrcError::Protocol("No CAP subcommand".into()));
        }

        match message.params[0].as_str() {
            "LS" => {
                // Send supported capabilities immediately and flush
                let caps = "server-time extended-join multi-prefix message-tags";
                self.write_raw(format!("CAP * LS :{}\r\n", caps).as_bytes()).await?;
                self.write_raw(b"").await?; // Empty write to force flush
                self.cap_negotiating = true;
            }
            "REQ" => {
                if message.params.len() < 2 {
                    return Err(IrcError::Protocol("No capabilities requested".into()));
                }
                // Send ACK immediately and flush
                self.write_raw(format!("CAP * ACK :{}\r\n", message.params[1]).as_bytes()).await?;
                self.write_raw(b"").await?; // Empty write to force flush
            }
            "END" => {
                self.cap_negotiating = false;
            }
            _ => {
                return Err(IrcError::Protocol("Unknown CAP subcommand".into()));
            }
        }

        Ok(())
    }

    pub fn has_capability(&self, cap: &Capability) -> bool {
        self.enabled_capabilities.contains(cap)
    }
} 