use crate::error::{IrcError, IrcResult};
use crate::ts6::TS6Message;
use crate::ircv3::Capability;
use tracing::{debug, warn};
use super::Client;

impl Client {
    pub(crate) async fn handle_cap(&mut self, message: TS6Message) -> IrcResult<()> {
        debug!("Handling CAP command: {:?}", message);

        match message.params.get(0).map(|s| s.as_str()) {
            Some("LS") => {
                // Set cap negotiating flag
                self.cap_negotiating = true;

                // Initialize available capabilities if not already done
                if self.available_capabilities.is_empty() {
                    self.available_capabilities.insert(Capability::MultiPrefix);
                    self.available_capabilities.insert(Capability::ExtendedJoin);
                    self.available_capabilities.insert(Capability::ServerTime);
                    self.available_capabilities.insert(Capability::MessageTags);
                }

                // Send CAP LS response with all available capabilities
                let caps = self.available_capabilities.iter()
                    .map(|cap| cap.to_string())
                    .collect::<Vec<_>>()
                    .join(" ");

                debug!("Sending CAP LS response: {}", caps);
                
                // Format the response correctly - the caps list needs to be a single parameter
                let response = format!("CAP * LS :{}", caps);
                self.write_raw(response.as_bytes()).await?;

                // Wait for client to send CAP REQ
                return Ok(());
            }
            Some("REQ") => {
                if let Some(caps_param) = message.params.get(1) {
                    let requested_caps: Vec<&str> = caps_param.split_whitespace().collect();
                    debug!("Client {} requested capabilities: {}", self.id, caps_param);

                    // Check if all requested capabilities are available
                    let mut valid = true;
                    for cap in &requested_caps {
                        if !self.available_capabilities.iter().any(|c| c.to_string() == *cap) {
                            valid = false;
                            break;
                        }
                    }

                    if valid {
                        // Enable requested capabilities
                        for cap in requested_caps {
                            if let Ok(cap) = cap.parse() {
                                self.enabled_capabilities.insert(cap);
                            }
                        }
                        
                        debug!("Acknowledging capabilities for client {}: {}", self.id, caps_param);
                        let response = format!("CAP * ACK :{}", caps_param);
                        self.write_raw(response.as_bytes()).await?;
                    } else {
                        debug!("Rejecting invalid capabilities for client {}: {}", self.id, caps_param);
                        let response = format!("CAP * NAK :{}", caps_param);
                        self.write_raw(response.as_bytes()).await?;
                    }
                }
                // Wait for client to send CAP END
                return Ok(());
            }
            Some("END") => {
                debug!("Ending CAP negotiation for client {}", self.id);
                self.cap_negotiating = false;
                
                // Check if we can complete registration
                if self.nickname.is_some() && self.username.is_some() && !self.registered {
                    debug!("Client {} has nick and user, completing registration", self.id);
                    self.complete_registration().await?;
                } else {
                    debug!("Client {} not ready for registration: nick={:?}, user={:?}", 
                        self.id, self.nickname, self.username);
                }
                return Ok(());
            }
            Some("LIST") => {
                let caps: String = self.enabled_capabilities
                    .iter()
                    .map(|cap| cap.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                
                debug!("Listing enabled capabilities: {}", caps);
                let msg = TS6Message::new("CAP".to_string(), vec!["*".to_string(), "LIST".to_string(), caps]);
                self.send_message(&msg).await?;
            }
            Some(subcmd) => {
                warn!("Invalid CAP subcommand: {}", subcmd);
                return Err(IrcError::Protocol("Invalid CAP subcommand".into()));
            }
            None => {
                warn!("Missing CAP subcommand");
                return Err(IrcError::Protocol("Missing CAP subcommand".into()));
            }
        }
        Ok(())
    }

    pub fn has_capability(&self, cap: &Capability) -> bool {
        self.enabled_capabilities.contains(cap)
    }
} 