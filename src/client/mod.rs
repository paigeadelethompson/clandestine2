// Submodules
// mod handler;
mod registration;
mod capability;
mod commands;

// Standard library imports
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::collections::{HashSet, VecDeque};
use std::time::{Duration, Instant};
use std::net::{SocketAddr, IpAddr};

// Tokio imports
use tokio::sync::Mutex;
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio::net::TcpStream;
use tokio::io::{AsyncWriteExt, AsyncBufReadExt, BufReader};
use tokio::task::JoinHandle;
use tokio::sync::broadcast;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

// External crate imports
use tracing::{debug, error, info, warn};
use chrono::Utc;
use regex::Regex;

// Internal crate imports
use crate::server::Server;
use crate::ircv3::Capability;
use crate::ts6::{TS6Message, parser::parse_message};
use crate::error::{IrcError, IrcResult};
use crate::config::{ServerConfig, HostmaskConfig};
use crate::channel::Channel;

// Re-exports
// pub use handler::*;
pub use registration::*;
pub use capability::*;
pub use commands::*;

// Static counter for client IDs
static NEXT_CLIENT_ID: AtomicU32 = AtomicU32::new(1);

fn generate_client_id() -> u32 {
    NEXT_CLIENT_ID.fetch_add(1, Ordering::Relaxed)
}

pub struct Client {
    id: u32,
    nickname: Option<String>,
    username: Option<String>,
    hostname: String,
    ip_addr: IpAddr,
    registered: bool,
    cap_negotiating: bool,
    enabled_capabilities: HashSet<Capability>,
    available_capabilities: HashSet<Capability>,
    account: Option<String>,
    realname: Option<String>,
    server_name: String,
    server: Arc<Server>,
    last_ping: Option<Instant>,
    last_pong: Option<Instant>,
    recvq: VecDeque<Vec<u8>>,
    ping_timer: Option<JoinHandle<()>>,
    tx: UnboundedSender<Vec<u8>>,         // For immediate writes
    sendq_tx: UnboundedSender<Vec<u8>>,   // For queued messages
    pong_tx: broadcast::Sender<()>,
    modes: HashSet<char>,
    ping_interval: Duration,
    ping_timeout: Duration,
}

impl Client {
    const MAX_SENDQ: usize = 40960; // 40KB
    const MAX_RECVQ: usize = 8192;  // 8KB

    pub fn new(writer: OwnedWriteHalf, addr: SocketAddr, server_name: String, server: Arc<Server>) -> Self {
        debug!("Creating new client connection from {}", addr);
        
        let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (sendq_tx, mut sendq_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (pong_tx, _) = broadcast::channel(16);

        // Get config values before moving server
        let ping_interval = Duration::from_secs(server.config.timeouts.ping_interval);
        let ping_timeout = Duration::from_secs(server.config.timeouts.ping_timeout);

        // Spawn writer task that handles both immediate and queued messages
        tokio::spawn(async move {
            let mut writer = writer;
            let mut current_sendq_size = 0;
            
            loop {
                tokio::select! {
                    // Handle immediate messages
                    msg = rx.recv() => {
                        match msg {
                            Some(msg) => {
                                debug!("Writer task: Got immediate message to send: {:?}", String::from_utf8_lossy(&msg));
                                if let Err(e) = writer.write_all(&msg).await {
                                    error!("Failed to write to stream: {}", e);
                                    break;
                                }
                                if let Err(e) = writer.flush().await {
                                    error!("Failed to flush stream: {}", e);
                                    break;
                                }
                            }
                            None => break, // Channel closed
                        }
                    }
                    
                    // Handle queued messages
                    msg = sendq_rx.recv() => {
                        match msg {
                            Some(msg) => {
                                if current_sendq_size + msg.len() <= Self::MAX_SENDQ {
                                    current_sendq_size += msg.len();
                                    debug!("Writer task: Got queued message to send: {:?}", String::from_utf8_lossy(&msg));
                                    if let Err(e) = writer.write_all(&msg).await {
                                        error!("Failed to write to stream: {}", e);
                                        break;
                                    }
                                    if let Err(e) = writer.flush().await {
                                        error!("Failed to flush stream: {}", e);
                                        break;
                                    }
                                    current_sendq_size = current_sendq_size.saturating_sub(msg.len());
                                } else {
                                    debug!("Dropping message due to sendq full");
                                }
                            }
                            None => break, // Channel closed
                        }
                    }
                }
            }
            debug!("Writer task: Channel closed, exiting");
        });

        let mut client = Self {
            id: generate_client_id(),
            nickname: None,
            username: None,
            hostname: addr.ip().to_string(),
            ip_addr: addr.ip(),
            registered: false,
            cap_negotiating: false,
            enabled_capabilities: HashSet::new(),
            available_capabilities: HashSet::new(),
            account: None,
            realname: None,
            server_name,
            server: server.clone(),
            last_ping: None,
            last_pong: None,
            recvq: VecDeque::new(),
            ping_timer: None,
            tx,
            sendq_tx,
            pong_tx,
            modes: HashSet::new(),
            ping_interval,
            ping_timeout,
        };
        
        client
    }

    fn start_ping_timer(&mut self) {
        let client_id = self.id;
        let tx = self.sendq_tx.clone();  // Use sendq_tx instead of tx for PINGs
        let mut pong_rx = self.pong_tx.subscribe();
        let server_name = self.server_name.clone();
        let ping_interval = self.ping_interval;
        let ping_timeout = self.ping_timeout;
        
        self.ping_timer = Some(tokio::spawn(async move {
            let mut interval = tokio::time::interval(ping_interval);
            interval.tick().await; // Skip first tick
            
            let mut last_ping = None;
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let now = Instant::now();
                        
                        // Only check timeout if we've sent a ping
                        if let Some(ping_time) = last_ping {
                            if now.duration_since(ping_time) > ping_timeout {
                                debug!("Client {} ping timeout - no PONG received", client_id);
                                let timeout_msg = "ERROR :Ping timeout\r\n".as_bytes().to_vec();
                                if tx.send(timeout_msg).is_err() {
                                    debug!("Failed to send timeout message - channel closed");
                                }
                                break;
                            }
                        }

                        // Send a new ping - add colon before param to match client's PONG format
                        debug!("Server sending PING to client {}", client_id);
                        let ping_msg = format!(":{} PING :{}\r\n", server_name, server_name);
                        match tx.send(ping_msg.into_bytes()) {
                            Ok(_) => {
                                debug!("Successfully queued PING message for client {}", client_id);
                                last_ping = Some(now);
                            }
                            Err(e) => {
                                debug!("Failed to send PING message: {} - channel closed", e);
                                break;
                            }
                        }
                    }
                    
                    Ok(_) = pong_rx.recv() => {
                        debug!("Received PONG update for client {}", client_id);
                        last_ping = None; // Reset ping timer when we get a PONG
                    }
                }
            }
            debug!("Ping timer task exiting for client {}", client_id);
        }));
    }

    pub async fn send_message(&self, message: &TS6Message) -> IrcResult<()> {
        let msg_string = message.to_string();
        debug!("Sending message to client {}: {:?}", self.id, msg_string);
        
        let mut data = msg_string.into_bytes();
        data.extend_from_slice(b"\r\n");
        
        // Use sendq for normal messages
        self.sendq_tx.send(data).map_err(|_| {
            let err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "Channel closed");
            IrcError::Io(err)
        })?;

        debug!("Successfully queued message for client {}", self.id);
        Ok(())
    }

    pub async fn write_raw(&self, data: &[u8]) -> IrcResult<()> {
        let mut data = data.to_vec();
        if !data.ends_with(b"\r\n") {
            data.extend_from_slice(b"\r\n");
        }
        
        // Use immediate channel for raw writes
        self.tx.send(data).map_err(|_| {
            let err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "Channel closed");
            IrcError::Io(err)
        })?;
        Ok(())
    }

    pub fn is_registered(&self) -> bool {
        self.registered
    }

    pub fn get_mask(&self) -> String {
        format!("{}!{}@{}", 
            self.nickname.as_ref().unwrap_or(&"*".to_string()),
            self.username.as_ref().unwrap_or(&"*".to_string()),
            self.hostname
        )
    }

    pub fn get_prefix(&self) -> String {
        if let (Some(nick), Some(user)) = (self.nickname.as_ref(), self.username.as_ref()) {
            format!("{}!{}@{}", nick, user, self.hostname)
        } else {
            // This should never happen for registered users, but just in case
            format!("unknown@{}", self.hostname)
        }
    }

    pub async fn send_error(&self, msg: &str) -> IrcResult<()> {
        let error_msg = format!("ERROR :{}\r\n", msg);
        self.write_raw(error_msg.as_bytes()).await
    }

    pub async fn send_numeric(&self, numeric: u16, params: &[&str]) -> IrcResult<()> {
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

    pub fn get_username(&self) -> Option<&String> {
        self.username.as_ref()
    }

    pub fn get_nickname(&self) -> Option<&String> {
        debug!("Getting nickname for client {}: {:?}", self.id, self.nickname);
        if let Some(ref nick) = self.nickname {
            debug!("Found nickname {} for client {}", nick, self.id);
        }
        self.nickname.as_ref()
    }

    pub fn get_realname(&self) -> Option<&String> {
        self.realname.as_ref()
    }

    pub fn get_hostname(&self) -> &str {
        &self.hostname
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn get_account(&self) -> Option<&String> {
        self.account.as_ref()
    }

    pub fn get_ip(&self) -> IpAddr {
        self.ip_addr
    }

    pub async fn cleanup(&mut self) {
        debug!("Cleaning up client {}", self.id);
        
        // Cancel the ping timer if it exists
        if let Some(timer) = self.ping_timer.take() {
            timer.abort();
        }

        // Remove from all channels
        if let Some(ref nick) = self.nickname {
            let channels = self.server.get_client_channels(self.id).await;
            for channel in channels {
                let quit_msg = TS6Message::with_source(
                    self.get_prefix(),
                    "QUIT".to_string(),
                    vec!["Connection closed".to_string()]
                );
                self.server.broadcast_to_channel(&channel, &quit_msg, Some(self.id)).await.ok();
                self.server.remove_from_channel(&channel, self.id).await.ok();
            }
        }

        // Clear any remaining queues
        self.recvq.clear();

        // Close the writer channel
        drop(self.tx.clone());

        debug!("Cleanup complete for client {}", self.id);
    }

    pub async fn handle_connection_with_reader(&mut self, reader: OwnedReadHalf) -> IrcResult<()> {
        let mut reader = BufReader::new(reader);
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

    pub async fn handle_message(&mut self, message: TS6Message) -> IrcResult<()> {
        debug!("Handling message: {:?}", message);
        
        match message.command.as_str() {
            // CAP must be handled first
            "CAP" => self.handle_cap(message).await,
            
            // During CAP negotiation, only allow CAP and QUIT
            cmd if self.cap_negotiating => {
                match cmd {
                    "QUIT" => self.handle_quit(message).await,
                    _ => {
                        warn!("Client {} sent {} during CAP negotiation", self.id, cmd);
                        Err(IrcError::Protocol("Must complete capability negotiation first (CAP END)".into()))
                    }
                }
            }

            // After CAP END, allow registration commands
            "NICK" => self.handle_nick(message).await,
            "USER" => self.handle_user(message).await,
            "QUIT" => self.handle_quit(message).await,
            "JOIN" => {
                debug!("Received JOIN command with params: {:?}", message.params);
                self.handle_join(message).await
            },
            "WHOIS" => {
                debug!("Received WHOIS command with params: {:?}", message.params);
                self.handle_whois(message).await
            },
            "PING" => self.handle_ping(message).await,
            "PONG" => self.handle_pong(message).await,
            "MODE" => self.handle_mode(message).await,
            "PRIVMSG" => self.handle_privmsg(message).await,
            "NOTICE" => self.handle_notice(message).await,
            "MOTD" => self.handle_motd(message).await,
            "LUSERS" => self.handle_lusers(message).await,
            "VERSION" => self.handle_version(message).await,
            "ADMIN" => self.handle_admin(message).await,
            "INFO" => self.handle_info(message).await,
            cmd => {
                warn!("Unknown command from client {}: {}", self.id, cmd);
                self.send_numeric(421, &[&message.command, "Unknown command"]).await
            }
        }
    }
} 