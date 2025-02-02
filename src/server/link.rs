use crate::error::{IrcError, IrcResult};
use crate::ts6::TS6Message;
use crate::ts6::parser::parse_message;
use tokio::net::TcpStream;
use tokio::io::{BufReader, AsyncBufReadExt, AsyncWriteExt, split};
use std::sync::Arc;
use tracing::{debug, info, warn};
use std::io;

pub struct ServerLink {
    name: String,
    sid: String,  // Server ID in TS6 format (3 chars)
    description: String,
    password: String,
    incoming: bool,
    capabilities: Vec<String>,
    stream: Option<TcpStream>,
    writer: Option<tokio::io::WriteHalf<TcpStream>>,
}

impl ServerLink {
    pub fn new(name: String, sid: String, description: String, password: String) -> Self {
        Self {
            name,
            sid,
            description,
            password,
            incoming: false,
            capabilities: vec![
                "QS".to_string(),     // Quit Storm
                "ENCAP".to_string(),  // Encapsulation
                "TB".to_string(),     // Topic Burst
                "SAVE".to_string(),   // SAVE nickname
                "SERVICES".to_string(), // Services support
            ],
            stream: None,
            writer: None,
        }
    }

    pub async fn handle_connection(&mut self, stream: TcpStream) -> IrcResult<()> {
        // Take ownership of the stream directly
        let (reader, writer) = split(stream);
        self.writer = Some(writer);

        // Initial handshake
        self.send_pass().await?;
        self.send_capab().await?;
        self.send_server().await?;
        
        // Start burst
        self.send_burst().await?;
        
        // Handle incoming messages
        let mut buf_reader = BufReader::new(reader);
        let mut line = String::new();

        while buf_reader.read_line(&mut line).await? > 0 {
            let msg = match parse_message(&line) {
                Some(msg) => msg,
                None => {
                    line.clear();
                    continue;
                }
            };

            self.handle_message(msg).await?;
            line.clear();
        }

        Ok(())
    }

    async fn send_pass(&mut self) -> IrcResult<()> {
        let pass_msg = TS6Message::new(
            "PASS".to_string(),
            vec![
                self.password.clone(),
                "TS".to_string(),
                "6".to_string(),
                self.sid.clone(),
            ],
        );
        self.send_message(&pass_msg).await
    }

    async fn send_capab(&mut self) -> IrcResult<()> {
        let capab_msg = TS6Message::new(
            "CAPAB".to_string(),
            vec![self.capabilities.join(" ")],
        );
        self.send_message(&capab_msg).await
    }

    async fn send_server(&mut self) -> IrcResult<()> {
        let server_msg = TS6Message::new(
            "SERVER".to_string(),
            vec![
                self.name.clone(),
                "1".to_string(), // Hopcount
                self.description.clone(),
            ],
        );
        self.send_message(&server_msg).await
    }

    async fn send_burst(&mut self) -> IrcResult<()> {
        // Send all local users
        self.send_users_burst().await?;
        
        // Send all channels
        self.send_channels_burst().await?;
        
        // End of burst
        let eob_msg = TS6Message::new("EOB".to_string(), vec![]);
        self.send_message(&eob_msg).await
    }

    async fn send_message(&mut self, message: &TS6Message) -> IrcResult<()> {
        if let Some(writer) = &mut self.writer {
            let msg = format!("{}\r\n", message.to_string());
            writer.write_all(msg.as_bytes()).await?;
            writer.flush().await?;
            Ok(())
        } else {
            Err(IrcError::Protocol("Not connected".into()))
        }
    }

    async fn send_users_burst(&mut self) -> IrcResult<()> {
        // TODO: Implement user burst
        // For each local user:
        // UID <nickname> <hopcount> <timestamp> <username> <hostname> <uid> <modes> <realname>
        Ok(())
    }

    async fn send_channels_burst(&mut self) -> IrcResult<()> {
        // TODO: Implement channel burst
        // For each channel:
        // SJOIN <timestamp> <channel> <modes> :[<prefix>]<uid> ...
        Ok(())
    }

    async fn handle_message(&mut self, message: TS6Message) -> IrcResult<()> {
        match message.command.as_str() {
            "PING" => {
                let pong = TS6Message::new(
                    "PONG".to_string(),
                    message.params
                );
                self.send_message(&pong).await?;
            }
            "SQUIT" => {
                // Handle server quit
                warn!("Server {} quit: {}", 
                    message.params.get(0).unwrap_or(&"unknown".to_string()),
                    message.params.get(1).unwrap_or(&"No reason given".to_string())
                );
                return Err(IrcError::Protocol("Server quit".into()));
            }
            // Add other message handlers
            _ => {
                debug!("Unhandled server message: {:?}", message);
            }
        }
        Ok(())
    }
} 