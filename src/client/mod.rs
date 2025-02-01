use crate::ts6::parser::parse_message;
use crate::error::{IrcError, IrcResult};
use crate::ts6::TS6Message;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, AsyncReadExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};
use std::sync::atomic::{AtomicU32, Ordering};
use crate::ircv3::Capability;
use std::collections::HashSet;
use chrono::Utc;

static NEXT_CLIENT_ID: AtomicU32 = AtomicU32::new(1);

fn generate_client_id() -> u32 {
    NEXT_CLIENT_ID.fetch_add(1, Ordering::Relaxed)
}

pub struct Client {
    pub(crate) id: u32,
    nickname: Option<String>,
    username: Option<String>,
    hostname: String,
    registered: bool,
    stream: Arc<Mutex<TcpStream>>,
    cap_negotiating: bool,
    enabled_capabilities: HashSet<Capability>,
    available_capabilities: HashSet<Capability>,
    account: Option<String>,
    realname: Option<String>,
    server_name: String,
}

impl Client {
    pub fn new(stream: TcpStream, addr: std::net::SocketAddr, server_name: String) -> Self {
        debug!("Creating new client connection from {}", addr);
        let mut available_capabilities = HashSet::new();
        // Add supported capabilities
        available_capabilities.insert(Capability::MultiPrefix);
        available_capabilities.insert(Capability::ExtendedJoin);
        available_capabilities.insert(Capability::ServerTime);
        available_capabilities.insert(Capability::MessageTags);

        Self {
            id: generate_client_id(),
            nickname: None,
            username: None,
            hostname: addr.ip().to_string(),
            registered: false,
            stream: Arc::new(Mutex::new(stream)),
            cap_negotiating: false,
            enabled_capabilities: HashSet::new(),
            available_capabilities,
            account: None,
            realname: None,
            server_name,
        }
    }

    pub async fn send_message(&self, message: &TS6Message) -> IrcResult<()> {
        let msg_string = message.to_string();
        debug!("Sending message to client {}: {:?}", self.id, msg_string);
        
        let mut stream = self.stream.lock().await;
        stream.write_all(msg_string.as_bytes()).await.map_err(|e| {
            error!("Failed to send message to client {}: {}", self.id, e);
            IrcError::Io(e)
        })?;
        stream.write_all(b"\r\n").await.map_err(|e| {
            error!("Failed to send line ending to client {}: {}", self.id, e);
            IrcError::Io(e)
        })?;
        stream.flush().await.map_err(|e| {
            error!("Failed to flush stream for client {}: {}", self.id, e);
            IrcError::Io(e)
        })?;

        debug!("Successfully sent message to client {}", self.id);
        Ok(())
    }

    pub async fn handle_connection(&mut self) -> IrcResult<()> {
        debug!("Starting connection handler for client {}", self.id);
        loop {
            let data = {
                let mut stream = self.stream.lock().await;
                let mut reader = BufReader::new(&mut *stream);
                let mut buffer = [0u8; 512];
                
                match reader.read(&mut buffer).await {
                    Ok(0) => return Ok(()),
                    Ok(n) => String::from_utf8_lossy(&buffer[..n]).into_owned(),
                    Err(e) => return Err(IrcError::Io(e)),
                }
            }; // stream lock is dropped here
            
            // Process messages outside the stream lock
            for line in data.lines() {
                if let Some(message) = parse_message(line) {
                    if let Err(e) = self.handle_message(message).await {
                        error!("Error handling message: {}", e);
                    }
                }
            }
        }
    }

    async fn handle_message(&mut self, message: TS6Message) -> IrcResult<()> {
        match message.command.as_str() {
            "CAP" => self.handle_cap(message).await,
            "PING" => {
                let response = TS6Message::new(
                    "PONG".to_string(),
                    message.params.clone(),
                );
                self.send_message(&response).await
            }
            "PONG" => Ok(()),
            "NICK" => self.handle_nick(message).await,
            "USER" => self.handle_user(message).await,
            cmd if !self.registered => {
                warn!("Unregistered client {} sent command: {}", self.id, cmd);
                Err(IrcError::Protocol("Not registered".into()))
            }
            "QUIT" => self.handle_quit(message).await,
            "JOIN" => self.handle_join(message).await,
            cmd => {
                warn!("Unknown command from client {}: {}", self.id, cmd);
                self.send_numeric(421, &[&message.command, "Unknown command"]).await
            }
        }
    }

    async fn handle_nick(&mut self, message: TS6Message) -> IrcResult<()> {
        if message.params.is_empty() {
            debug!("Client {} sent NICK with no parameters", self.id);
            return Err(IrcError::Protocol("No nickname provided".into()));
        }

        let new_nick = &message.params[0];
        debug!("Client {} attempting to set nickname to {}", self.id, new_nick);
        // TODO: Add nickname validation
        // TODO: Check for nickname collisions
        
        self.nickname = Some(new_nick.clone());
        debug!("Client {} nickname set to {}", self.id, new_nick);

        if self.is_registered() {
            // Send nickname change notification
            let msg = TS6Message::with_source(
                self.get_prefix(),
                "NICK".to_string(),
                vec![new_nick.clone()]
            );
            self.send_message(&msg).await?;
        } else {
            debug!("Client {} attempting registration after NICK", self.id);
            self.try_register().await?;
        }

        Ok(())
    }

    async fn handle_user(&mut self, message: TS6Message) -> IrcResult<()> {
        debug!("Client {} USER command with params: {:?}", self.id, message.params);
        if message.params.len() < 4 {
            return Err(IrcError::Protocol("Invalid USER command".into()));
        }

        if self.username.is_some() {
            return Err(IrcError::Protocol("Cannot change USER once registered".into()));
        }

        self.username = Some(message.params[0].clone());
        self.realname = Some(message.params[3].clone());
        debug!("Client {} set username={:?}, realname={:?}", 
               self.id, self.username, self.realname);

        debug!("Client {} attempting registration after USER", self.id);
        self.try_register().await?;
        Ok(())
    }

    async fn handle_quit(&mut self, message: TS6Message) -> IrcResult<()> {
        let quit_message = message.params.first()
            .map(|s| s.as_str())
            .unwrap_or("Client quit");

        info!("Client {} quit: {}", self.id, quit_message);
        
        // Send quit message to other clients
        let msg = TS6Message::with_source(
            self.get_prefix(),
            "QUIT".to_string(),
            vec![quit_message.to_string()]
        );
        self.send_message(&msg).await?;
        
        Ok(())
    }

    async fn handle_cap(&mut self, message: TS6Message) -> IrcResult<()> {
        if message.params.is_empty() {
            return Err(IrcError::Protocol("Invalid CAP command".into()));
        }

        debug!("Handling CAP command: {:?}", message);

        match message.params[0].to_uppercase().as_str() {
            "LS" => {
                self.cap_negotiating = true;
                let caps: String = self.available_capabilities
                    .iter()
                    .map(|cap| cap.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                
                debug!("Sending CAP LS response: {}", caps);
                // For CAP LS 302, we need to send just the capabilities list
                let msg = if message.params.get(1) == Some(&"302".to_string()) {
                    TS6Message::new("CAP".to_string(), vec!["*".to_string(), "LS".to_string(), caps])
                } else {
                    TS6Message::new("CAP".to_string(), vec!["*".to_string(), "LS".to_string(), caps])
                };
                self.send_message(&msg).await?;
            }
            "REQ" => {
                if message.params.len() < 2 {
                    return Err(IrcError::Protocol("Invalid CAP REQ command".into()));
                }

                let requested_caps = message.params[1].clone();
                debug!("Client requested capabilities: {}", requested_caps);

                let requested_caps: Vec<_> = requested_caps
                    .split_whitespace()
                    .filter_map(Capability::from_str)
                    .collect();

                // Check if all requested capabilities are available
                let all_available = requested_caps.iter()
                    .all(|cap| self.available_capabilities.contains(cap));

                if all_available {
                    // Enable the capabilities
                    for cap in requested_caps {
                        self.enabled_capabilities.insert(cap);
                    }

                    let caps = message.params[1].clone();
                    debug!("Acknowledging capabilities: {}", caps);
                    let msg = TS6Message::new("CAP".to_string(), vec!["*".to_string(), "ACK".to_string(), caps]);
                    self.send_message(&msg).await?;
                } else {
                    debug!("Rejecting capabilities: {}", message.params[1]);
                    let msg = TS6Message::new("CAP".to_string(), vec!["*".to_string(), "NAK".to_string(), message.params[1].clone()]);
                    self.send_message(&msg).await?;
                }
            }
            "END" => {
                debug!("Ending capability negotiation");
                self.cap_negotiating = false;
                // Don't try to register here - wait for NICK and USER
            }
            "LIST" => {
                let caps: String = self.enabled_capabilities
                    .iter()
                    .map(|cap| cap.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                
                debug!("Listing enabled capabilities: {}", caps);
                let msg = TS6Message::new("CAP".to_string(), vec!["*".to_string(), "LIST".to_string(), caps]);
                self.send_message(&msg).await?;
            }
            subcmd => {
                warn!("Invalid CAP subcommand: {}", subcmd);
                return Err(IrcError::Protocol("Invalid CAP subcommand".into()));
            }
        }

        Ok(())
    }

    async fn send_numeric(&self, numeric: u16, params: &[&str]) -> IrcResult<()> {
        let numeric_str = format!("{:03}", numeric);
        let mut message_params = vec![];
        
        if let Some(nick) = &self.nickname {
            message_params.push(nick.clone());
        } else {
            message_params.push("*".to_string());
        }
        
        message_params.extend(params.iter().map(|&s| s.to_string()));
        
        let mut message = TS6Message::new(numeric_str, message_params);
        // Add server name as source for numeric replies
        message.source = Some(self.server_name.clone());
        self.send_message(&message).await
    }

    fn is_registered(&self) -> bool {
        self.registered
    }

    async fn try_register(&mut self) -> IrcResult<()> {
        debug!(
            "Checking registration: nick={:?}, user={:?}, registered={}, cap_negotiating={}",
            self.nickname, self.username, self.registered, self.cap_negotiating
        );

        if self.nickname.is_some() && self.username.is_some() && !self.registered && !self.cap_negotiating {
            self.registered = true;
            info!("Client {} registered as {}", self.id, self.nickname.as_ref().unwrap());
            
            // Send registration messages in the required order
            // 001 RPL_WELCOME
            self.send_numeric(001, &[&format!("Welcome to {} {}", 
                self.server_name,
                self.get_prefix()
            )]).await?;

            // 002 RPL_YOURHOST
            self.send_numeric(002, &[&format!("Your host is {}, running ircd-rs v0.1.0",
                self.server_name
            )]).await?;

            // 003 RPL_CREATED
            self.send_numeric(003, &[&format!("This server was created {}",
                chrono::Local::now().format("%Y-%m-%d")
            )]).await?;

            // 004 RPL_MYINFO
            self.send_numeric(004, &[
                &self.server_name,
                "ircd-rs-0.1.0",  // version
                "iowghraAsORTVSxNCWqBzvdHtGp", // user modes
                "bkloveqjfI",     // channel modes
                "bklov",          // channel modes that take parameters
            ]).await?;

            // 005 RPL_ISUPPORT
            self.send_numeric(005, &[
                "CHANTYPES=#",
                "EXCEPTS",
                "INVEX",
                "CHANMODES=eIbq,k,flj,CFLMPQScgimnprstuz",
                "CHANLIMIT=#:100",
                "PREFIX=(ov)@+",
                "MAXLIST=bqeI:100",
                "MODES=4",
                "NETWORK=ExampleNet",
                "STATUSMSG=@+",
                "CALLERID=g",
                "CASEMAPPING=rfc1459",
                ":are supported by this server"
            ]).await?;

            // Send MOTD after registration is complete
            self.send_numeric(375, &["- Message of the day"]).await?;
            self.send_numeric(372, &["- Welcome to IRCd-rs!"]).await?;
            self.send_numeric(376, &["End of /MOTD command."]).await?;
        }
        Ok(())
    }

    fn get_prefix(&self) -> String {
        if let Some(nick) = &self.nickname {
            if let Some(user) = &self.username {
                format!("{}!{}@{}", nick, user, self.hostname)
            } else {
                format!("{}!unknown@{}", nick, self.hostname)
            }
        } else {
            format!("unknown@{}", self.hostname)
        }
    }

    pub fn has_capability(&self, cap: &Capability) -> bool {
        self.enabled_capabilities.contains(cap)
    }

    async fn send_message_with_time(&self, message: &mut TS6Message) -> IrcResult<()> {
        if self.has_capability(&Capability::ServerTime) {
            let timestamp = Utc::now().format("@time=%Y-%m-%dT%H:%M:%S.%3fZ ");
            message.tags.insert("time".to_string(), timestamp.to_string());
        }
        self.send_message(message).await
    }

    async fn handle_join(&mut self, message: TS6Message) -> IrcResult<()> {
        if message.params.is_empty() {
            return Err(IrcError::Protocol("No channel specified".into()));
        }

        let channel_name = &message.params[0];
        // TODO: Add channel validation
        
        let mut response = TS6Message::with_source(
            self.get_prefix(),
            "JOIN".to_string(),
            if self.has_capability(&Capability::ExtendedJoin) {
                vec![
                    channel_name.clone(),
                    self.account.as_deref().unwrap_or("*").to_string(),
                    self.realname.clone().unwrap_or_default(),
                ]
            } else {
                vec![channel_name.clone()]
            }
        );

        self.send_message_with_time(&mut response).await?;

        // Send channel modes if we have multi-prefix capability
        if self.has_capability(&Capability::MultiPrefix) {
            // Example: Send all prefix modes for users
            let mode_response = TS6Message::new(
                "MODE".to_string(),
                vec![channel_name.clone(), "+ov nick1 nick2".to_string()]
            );
            self.send_message(&mode_response).await?;
        }

        Ok(())
    }

    async fn handle_names(&mut self, channel: &str, members: &[(String, Vec<char>)]) -> IrcResult<()> {
        let mut prefixes = Vec::new();
        for (nick, modes) in members {
            let mut prefix = String::new();
            if self.has_capability(&Capability::MultiPrefix) {
                // With multi-prefix, show all prefix modes
                for mode in modes {
                    match mode {
                        'o' => prefix.push('@'),
                        'v' => prefix.push('+'),
                        'h' => prefix.push('%'),
                        'a' => prefix.push('&'),
                        'q' => prefix.push('~'),
                        _ => {}
                    }
                }
            } else {
                // Without multi-prefix, show only the highest prefix
                if let Some(mode) = modes.first() {
                    match mode {
                        'q' => prefix.push('~'),
                        'a' => prefix.push('&'),
                        'o' => prefix.push('@'),
                        'h' => prefix.push('%'),
                        'v' => prefix.push('+'),
                        _ => {}
                    }
                }
            }
            prefixes.push(format!("{}{}", prefix, nick));
        }

        let mut response = TS6Message::new(
            "353".to_string(),
            vec![
                self.nickname.clone().unwrap_or_else(|| "*".to_string()),
                "=".to_string(),
                channel.to_string(),
                prefixes.join(" ")
            ]
        );

        self.send_message_with_time(&mut response).await
    }
} 