use std::collections::{HashSet, VecDeque};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use chrono::Utc;
use regex::Regex;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

pub use capability::*;
pub use registration::*;

use crate::channel::Channel;
use crate::config::{HostmaskConfig, ServerConfig};
use crate::error::{IrcError, IrcResult};
use crate::ircv3::Capability;
use crate::server::Server;
use crate::ts6::{parser::parse_message, TS6Message};

mod registration;
mod capability;

#[cfg(test)]
mod tests;
mod ping;
mod message;
mod error;
mod numeric;
mod channel;
mod query;
mod server;
mod user;

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
                    vec!["Connection closed".to_string()],
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

    pub async fn handle_connection_with_reader(&mut self, mut reader: OwnedReadHalf) -> IrcResult<()> {
        let mut lines = BufReader::new(reader).lines();

        while let Some(line) = lines.next_line().await? {
            debug!("Received line from client {}: {}", self.id, line);

            // Parse the message - add & to borrow the line
            if let Ok(message) = parse_message(&line) {
                // Process the message
                self.handle_message(message).await?;
            } else {
                warn!("Failed to parse message from client {}: {}", self.id, line);
                // Optionally send an error to the client
                self.send_numeric(421, &["Unknown command"]).await?;
            }
        }

        Ok(())
    }

    pub fn set_nickname(&mut self, nickname: String) -> IrcResult<()> {
        debug!("Setting nickname for client {} to {}", self.id, nickname);
        self.nickname = Some(nickname);
        Ok(())
    }

    pub fn set_hostname(&mut self, hostname: String) {
        debug!("Setting hostname for client {} to {}", self.id, hostname);
        self.hostname = hostname;
    }
}
