use crate::ts6::parser::parse_message;
use crate::error::{IrcError, IrcResult};
use crate::ts6::TS6Message;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{debug, error, warn};
use super::Client;
use std::net::TcpStream;

impl Client {
    pub async fn handle_connection(&mut self) -> IrcResult<()> {
        debug!("Starting connection handler for client {}", self.id);
        
        // Create a new reader for the connection
        let mut reader = BufReader::new(TcpStream::connect(self.get_ip().to_string()).await?);
        let mut buffer = String::new();
        
        loop {
            buffer.clear();
            match reader.read_line(&mut buffer).await {
                Ok(0) => {
                    debug!("Client {} closed connection", self.id);
                    return Ok(());
                }
                Ok(_) => {
                    let line = buffer.trim();
                    if let Some(message) = parse_message(line) {
                        match self.handle_message(message).await {
                            Ok(_) => continue,
                            Err(e) => {
                                error!("Error handling message for client {}: {}", self.id, e);
                                if matches!(e, IrcError::Protocol(_)) {
                                    continue; // Continue on protocol errors
                                }
                                return Err(e); // Return on other errors
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Read error for client {}: {}", self.id, e);
                    return Err(IrcError::Io(e));
                }
            }
        }
    }

    async fn handle_message(&mut self, message: TS6Message) -> IrcResult<()> {
        match message.command.as_str() {
            "CAP" => self.handle_cap(message).await,
            "PING" => self.handle_ping(message).await,
            "PONG" => self.handle_pong(message).await,
            "NICK" => self.handle_nick(message).await,
            "USER" => self.handle_user(message).await,
            // ... etc
            cmd if !self.registered => {
                warn!("Unregistered client {} sent command: {}", self.id, cmd);
                Err(IrcError::Protocol("Not registered".into()))
            }
            cmd => {
                warn!("Unknown command from client {}: {}", self.id, cmd);
                self.send_numeric(421, &[&message.command, "Unknown command"]).await
            }
        }
    }
} 