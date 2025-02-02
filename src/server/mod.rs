pub mod link;

use crate::database::Database;
use crate::channel::Channel;
use crate::config::{ServerConfig, KLine, DLine, GLine, ILine, ServerLinkConfig};
use crate::error::{IrcError, IrcResult};
use crate::client::Client;
use crate::ts6::TS6Message;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::net::TcpStream;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use chrono::{DateTime, Utc};
use regex;
use std::time::Duration;
use std::sync::mpsc::{self, Sender, Receiver};
use crate::server::link::ServerLink;

pub struct Server {
    pub(crate) config: Arc<ServerConfig>,
    clients: Arc<RwLock<Vec<ClientId>>>,
    client_map: Arc<RwLock<HashMap<ClientId, Arc<Mutex<Client>>>>>,
    channels: Arc<RwLock<HashMap<String, Arc<RwLock<Channel>>>>>,
    database: Option<Arc<Database>>,
    nicknames: Arc<RwLock<HashMap<String, ClientId>>>,
    registration_timeouts: Arc<RwLock<HashMap<ClientId, tokio::time::Instant>>>,
    nickname_map: Arc<RwLock<HashMap<String, ClientId>>>,
    tx: mpsc::Sender<ServerMessage>,
    linked_servers: Arc<RwLock<HashMap<String, Arc<Mutex<ServerLink>>>>>,
}

type ClientId = u32;

#[derive(Default)]
pub struct ServerStats {
    pub visible_users: usize,
    pub invisible_users: usize,
    pub server_count: usize,
    pub oper_count: usize,
    pub channel_count: usize,
    pub local_users: usize,
    pub local_servers: usize,
    pub max_local_users: usize,
    pub global_users: usize,
    pub max_global_users: usize,
}

pub enum ServerMessage {
    WhoisLookup { 
        nickname: String, 
        respond_to: mpsc::Sender<Option<WhoisInfo>> 
    },
}

#[derive(Clone)]
pub struct WhoisInfo {
    pub nickname: String,
    pub username: String,
    pub hostname: String,
    pub realname: String,
}

impl Server {
    const REGISTRATION_TIMEOUT: Duration = Duration::from_secs(60);
    
    pub async fn new(config: ServerConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let database = if let Some(db_config) = &config.database {
            if db_config.persist_lines {
                Some(Arc::new(Database::new(&db_config.path).await?))
            } else {
                None
            }
        } else {
            None
        };

        let (tx, rx) = mpsc::channel();
        
        let server = Self {
            config: Arc::new(config),
            clients: Arc::new(RwLock::new(Vec::new())),
            client_map: Arc::new(RwLock::new(HashMap::new())),
            channels: Arc::new(RwLock::new(HashMap::new())),
            database,
            nicknames: Arc::new(RwLock::new(HashMap::new())),
            registration_timeouts: Arc::new(RwLock::new(HashMap::new())),
            nickname_map: Arc::new(RwLock::new(HashMap::new())),
            tx,
            linked_servers: Arc::new(RwLock::new(HashMap::new())),
        };

        // Load persisted lines if database is configured
        if let Some(db) = &server.database {
            server.load_persisted_lines(db).await?;
        }

        // Spawn the server task in a new thread since we're using std::sync::mpsc
        std::thread::spawn(move || {
            server_task(rx);
        });

        Ok(server)
    }

    async fn load_persisted_lines(&self, db: &Database) -> Result<(), Box<dyn std::error::Error>> {
        // Load lines from database and merge with config
        let mut access = self.config.access.clone();
        
        // Load and merge K-lines
        let db_klines = db.get_klines().await;
        access.klines.extend(db_klines);
        
        // Load and merge other line types...
        Ok(())
    }

    pub async fn add_kline(&self, kline: KLine) -> Result<(), Box<dyn std::error::Error>> {
        // Add to memory
        let mut access = self.config.access.clone();
        access.klines.push(kline.clone());
        
        Ok(())
    }

    // Similar methods for other line types...

    pub async fn run(&self) -> IrcResult<()> {
        let addr = format!("{}:{}", 
            self.config.server.bind_addr,
            self.config.server.port
        );
        
        let listener = TcpListener::bind(&addr).await.map_err(|e| {
            error!("Failed to bind to address {}: {}", addr, e);
            IrcError::Io(e)
        })?;
        
        info!("Server listening on {}", addr);

        loop {
            match listener.accept().await {
                Ok((socket, addr)) => {
                    info!("New connection from: {}", addr);
                    let server = Arc::new(self.clone());
                    
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(socket, server).await {
                            error!("Error handling connection from {}: {}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("Error accepting connection: {}", e);
                }
            }
        }
    }

    pub async fn broadcast_global(&self, message: &str) -> IrcResult<()> {
        debug!("Broadcasting global message: {}", message);
        // Implementation
        Ok(())
    }

    // Get a snapshot of all clients without holding locks
    pub async fn get_all_clients(&self) -> Vec<Arc<Mutex<Client>>> {
        self.client_map.read().await.values().cloned().collect()
    }

    // Fix get_channel_members
    pub async fn get_channel_members(&self, channel_name: &str) -> Vec<ClientId> {
        let channels = self.channels.read().await;
        if let Some(channel) = channels.get(channel_name) {
            let channel = channel.read().await;
            channel.get_members().iter().cloned().collect()
        } else {
            Vec::new()
        }
    }

    // Fix check_channel_membership
    pub async fn check_channel_membership(&self, channel_name: &str, client_id: u32) -> bool {
        let channels = self.channels.read().await;
        if let Some(channel) = channels.get(channel_name) {
            let channel = channel.read().await;
            channel.get_members().contains(&client_id)
        } else {
            false
        }
    }

    pub async fn broadcast_to_channel(&self, channel_name: &str, message: &TS6Message, skip_client: Option<u32>) -> IrcResult<()> {
        // Get member list without holding the lock
        let member_ids = self.get_channel_members(channel_name).await;

        // Send messages without holding any locks
        for client_id in member_ids {
            if Some(client_id) == skip_client {
                continue;
            }
            if let Some(client) = self.get_client(client_id).await {
                let _ = client.lock().await.send_message(message).await;
            }
        }

        Ok(())
    }

    pub async fn find_client_by_nick(&self, nickname: &str) -> Option<Arc<Mutex<Client>>> {
        let nickname_lower = nickname.to_lowercase();
        debug!("find_client_by_nick: Looking for nickname {} (lowercase: {})", nickname, nickname_lower);
        
        // First look up the client ID in the nickname map
        let client_id = {
            let nicknames = self.nickname_map.read().await;
            nicknames.get(&nickname_lower).copied()
        };

        // Then get the client from the client map if we found an ID
        if let Some(id) = client_id {
            let clients = self.client_map.read().await;
            return clients.get(&id).cloned();
        }

        debug!("find_client_by_nick: No match found for {}", nickname);
        None
    }

    pub async fn get_stats(&self) -> ServerStats {
        let mut stats = ServerStats::default();
        
        // Get snapshots without holding locks
        let client_count = {
            let clients = self.clients.read().await;
            clients.len()
        };
        
        let channel_count = {
            let channels = self.channels.read().await;
            channels.len()
        };
        
        stats.local_users = client_count;
        stats.global_users = client_count;
        stats.channel_count = channel_count;
        stats.max_local_users = client_count;
        stats.max_global_users = client_count;
        stats.server_count = 1;
        
        stats
    }

    // Fix get_or_create_channel
    pub async fn get_or_create_channel(&self, name: &str) -> Arc<RwLock<Channel>> {
        let mut channels = self.channels.write().await;
        if let Some(channel) = channels.get(name) {
            channel.clone()
        } else {
            let channel = Arc::new(RwLock::new(Channel::new(name.to_string())));
            channels.insert(name.to_string(), channel.clone());
            channel
        }
    }

    // Fix get_channel
    pub async fn get_channel(&self, name: &str) -> Option<Arc<RwLock<Channel>>> {
        let channels = self.channels.read().await;
        channels.get(name).cloned()
    }

    // Update get_client to use the client_map
    pub(crate) async fn get_client(&self, id: ClientId) -> Option<Arc<Mutex<Client>>> {
        let client_map = self.client_map.read().await;
        client_map.get(&id).cloned()
    }

    // Update add_client to store in both the list and map
    pub async fn add_client(&self, client: Arc<Mutex<Client>>) {
        let id = client.lock().await.id();
        let mut clients = self.clients.write().await;
        let mut client_map = self.client_map.write().await;
        
        clients.push(id);
        client_map.insert(id, client);
        debug!("Added client {} to server", id);
    }

    // Update remove_client to be more thorough
    pub async fn remove_client(&self, id: ClientId) {
        debug!("Removing client {} from server", id);
        
        // Remove from client list
        let mut clients = self.clients.write().await;
        if let Some(pos) = clients.iter().position(|&cid| cid == id) {
            clients.swap_remove(pos);
        }
        
        // Remove from client map
        let mut client_map = self.client_map.write().await;
        if client_map.remove(&id).is_some() {
            info!("Removed client {} from server", id);
        }
        
        // Could also clean up from channels here if needed
        debug!("Client {} cleanup completed", id);
    }

    pub async fn check_access(&self, client: &Client) -> Result<(), String> {
        // Check D-lines first (IP bans)
        if let Some(dline) = self.is_dlined(client).await {
            return Err(format!("D-lined: {}", dline.reason));
        }

        // Check K-lines
        if let Some(kline) = self.is_klined(client).await {
            return Err(format!("K-lined: {}", kline.reason));
        }

        // Check G-lines
        if let Some(gline) = self.is_glined(client).await {
            return Err(format!("G-lined: {}", gline.reason));
        }

        // Check I-lines (if no matching I-line, reject)
        if !self.has_iline(client).await {
            return Err("No matching I-line".to_string());
        }

        Ok(())
    }

    async fn is_dlined(&self, client: &Client) -> Option<&DLine> {
        let ip = client.get_ip();
        self.config.access.dlines.iter()
            .find(|dline| {
                dline.ip == ip && 
                !self.is_ban_expired(dline.set_time, dline.duration)
            })
    }

    async fn is_klined(&self, client: &Client) -> Option<&KLine> {
        let mask = client.get_mask();
        self.config.access.klines.iter()
            .find(|kline| {
                self.mask_match(&mask, &kline.mask) && 
                !self.is_ban_expired(kline.set_time, kline.duration)
            })
    }

    async fn is_glined(&self, client: &Client) -> Option<&GLine> {
        let mask = client.get_mask();
        self.config.access.glines.iter()
            .find(|gline| {
                self.mask_match(&mask, &gline.mask) && 
                !self.is_ban_expired(gline.set_time, gline.duration)
            })
    }

    async fn has_iline(&self, client: &Client) -> bool {
        let mask = client.get_mask();
        self.config.access.ilines.iter()
            .any(|iline| self.mask_match(&mask, &iline.mask))
    }

    pub async fn has_oline(&self, client: &Client) -> bool {
        let mask = client.get_mask();
        self.config.access.olines.iter()
            .any(|oline| self.mask_match(&mask, &oline.mask))
    }

    pub async fn is_ulined(&self, server_name: &str) -> bool {
        self.config.access.ulines.iter()
            .any(|uline| uline.server == server_name)
    }

    pub async fn has_aline(&self, client: &Client) -> bool {
        let mask = client.get_mask();
        self.config.access.alines.iter()
            .any(|aline| self.mask_match(&mask, &aline.mask))
    }

    fn is_ban_expired(&self, set_time: DateTime<Utc>, duration: i64) -> bool {
        if duration == 0 {
            return false; // Permanent ban
        }
        (Utc::now() - set_time).num_seconds() > duration
    }

    pub fn mask_match(&self, host: &str, mask: &str) -> bool {
        // Implement IRC-style mask matching
        // Convert mask to regex pattern and match
        let pattern = mask.replace("*", ".*")
            .replace("?", ".")
            .replace("[", "\\[")
            .replace("]", "\\]");
        regex::Regex::new(&format!("^{}$", pattern))
            .map(|re| re.is_match(host))
            .unwrap_or(false)
    }

    // Helper method to get client's hostmask
    fn get_client_mask(client: &Client) -> String {
        format!("{}!{}@{}", 
            client.get_nickname().map(|s| s.as_str()).unwrap_or("*"),
            client.get_username().map(|s| s.as_str()).unwrap_or("*"),
            client.get_hostname()
        )
    }

    pub async fn remove_from_channel(&self, channel_name: &str, client_id: u32) -> IrcResult<()> {
        debug!("Server removing client {} from channel {}", client_id, channel_name);
        
        if let Some(channel) = self.get_channel(channel_name).await {
            let mut channel = channel.write().await;
            channel.remove_member(client_id);
            debug!("Successfully removed client {} from channel {}", client_id, channel_name);
            Ok(())
        } else {
            debug!("Channel {} not found when removing client {}", channel_name, client_id);
            Err(IrcError::Protocol("No such channel".into()))
        }
    }

    pub async fn check_nickname(&self, nickname: &str) -> bool {
        let nicknames = self.nicknames.read().await;
        !nicknames.contains_key(&nickname.to_lowercase())
    }

    pub async fn register_nickname(&self, nickname: &str, client_id: ClientId) -> IrcResult<()> {
        let nickname_lower = nickname.to_lowercase();
        let mut nicknames = self.nickname_map.write().await;
        
        if nicknames.contains_key(&nickname_lower) {
            return Err(IrcError::Protocol("Nickname is already in use".into()));
        }

        debug!("Registering nickname {} for client {}", nickname, client_id);
        nicknames.insert(nickname_lower, client_id);
        Ok(())
    }

    pub async fn unregister_nickname(&self, nickname: &str) {
        let nickname_lower = nickname.to_lowercase();
        let mut nicknames = self.nickname_map.write().await;
        nicknames.remove(&nickname_lower);
    }

    async fn start_registration_timeout(&self, client_id: ClientId) {
        let mut timeouts = self.registration_timeouts.write().await;
        timeouts.insert(client_id, tokio::time::Instant::now());
        
        // Create a new Arc for the server
        let server = Arc::new(self.clone());
        
        tokio::spawn(async move {
            tokio::time::sleep(Self::REGISTRATION_TIMEOUT).await;
            server.check_registration_timeout(client_id).await;
        });
    }

    async fn check_registration_timeout(&self, client_id: ClientId) {
        if let Some(client) = self.get_client(client_id).await {
            let mut client = client.lock().await;
            if !client.is_registered() {
                client.send_message(&TS6Message::new(
                    "ERROR".to_string(),
                    vec!["Registration timeout".to_string()]
                )).await.ok();
                // Disconnect client
                self.remove_client(client_id).await;
            }
        }
    }

    // Fix get_channel_list
    pub async fn get_channel_list(&self) -> Vec<(String, usize, Option<String>)> {
        let channels = self.channels.read().await;
        let mut result = Vec::new();
        
        for (name, channel) in channels.iter() {
            let channel = channel.read().await;
            result.push((
                name.clone(),
                channel.get_members().len(),
                channel.get_topic()
            ));
        }
        
        result
    }

    // Fix get_client_channels
    pub async fn get_client_channels(&self, client_id: u32) -> Vec<String> {
        let mut client_channels = Vec::new();
        let channels = self.channels.read().await;
        
        for (channel_name, channel) in channels.iter() {
            let channel = channel.read().await;
            if channel.get_members().contains(&client_id) {
                client_channels.push(channel_name.clone());
            }
        }
        
        client_channels
    }

    pub async fn find_client_info(&self, nickname: &str) -> Option<WhoisInfo> {
        let (resp_tx, resp_rx) = mpsc::channel();
        
        self.tx.send(ServerMessage::WhoisLookup {
            nickname: nickname.to_string(),
            respond_to: resp_tx,
        }).expect("Server task died");

        resp_rx.recv().expect("Server task died")
    }

    pub async fn link_server(&self, config: ServerLinkConfig) -> IrcResult<()> {
        let server_link = ServerLink::new(
            config.name.clone(),
            config.sid,
            config.description,
            config.password,
        );

        // Connect to remote server
        let stream = TcpStream::connect(&config.address).await?;
        
        let server_link = Arc::new(Mutex::new(server_link));
        
        {
            let mut servers = self.linked_servers.write().await;
            servers.insert(config.name, Arc::clone(&server_link));
        }

        // Handle connection in separate task
        let server_link_clone = Arc::clone(&server_link);
        tokio::spawn(async move {
            if let Err(e) = server_link_clone.lock().await.handle_connection(stream).await {
                warn!("Server link error: {}", e);
            }
        });

        Ok(())
    }

    // Handle incoming server messages
    pub(crate) async fn handle_server_message(&self, msg: TS6Message) -> IrcResult<()> {
        match msg.command.as_str() {
            "PASS" => self.handle_server_pass(msg).await,
            "CAPAB" => self.handle_server_capab(msg).await,
            "SERVER" => self.handle_server_intro(msg).await,
            "SJOIN" => self.handle_server_join(msg).await,
            "SID" => self.handle_server_sid(msg).await,
            "PING" => self.handle_server_ping(msg).await,
            "PONG" => self.handle_server_pong(msg).await,
            "SQUIT" => self.handle_server_quit(msg).await,
            _ => Ok(()),
        }
    }

    async fn handle_server_pass(&self, msg: TS6Message) -> IrcResult<()> {
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

    async fn handle_server_capab(&self, msg: TS6Message) -> IrcResult<()> {
        // CAPAB capabilities...
        if msg.params.is_empty() {
            return Err(IrcError::Protocol("No capabilities specified".into()));
        }

        // TODO: Process capabilities
        Ok(())
    }

    async fn handle_server_intro(&self, msg: TS6Message) -> IrcResult<()> {
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

    async fn handle_server_join(&self, msg: TS6Message) -> IrcResult<()> {
        // SJOIN timestamp channel modes members
        if msg.params.len() < 4 {
            return Err(IrcError::Protocol("Invalid SJOIN parameters".into()));
        }

        // TODO: Process channel join with TS
        Ok(())
    }

    async fn handle_server_sid(&self, msg: TS6Message) -> IrcResult<()> {
        // SID name hopcount sid description
        if msg.params.len() < 4 {
            return Err(IrcError::Protocol("Invalid SID parameters".into()));
        }

        // TODO: Process server introduction
        Ok(())
    }

    async fn handle_server_ping(&self, msg: TS6Message) -> IrcResult<()> {
        // PING source [destination]
        if msg.params.is_empty() {
            return Err(IrcError::Protocol("No PING source".into()));
        }

        // Send PONG response
        let pong = TS6Message::new(
            "PONG".to_string(),
            vec![self.config.server.name.clone(), msg.params[0].clone()]
        );

        // TODO: Send to correct server
        Ok(())
    }

    async fn handle_server_pong(&self, _msg: TS6Message) -> IrcResult<()> {
        // PONG is handled by the individual server links
        Ok(())
    }

    async fn handle_server_quit(&self, msg: TS6Message) -> IrcResult<()> {
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

// Update handle_connection to ensure cleanup on any error
pub async fn handle_connection(stream: TcpStream, server: Arc<Server>) -> IrcResult<()> {
    let addr = stream.peer_addr()?;
    stream.set_nodelay(true)?;
    debug!("Starting new connection handler for {}", addr);
    
    // Split the stream
    let (reader, writer) = stream.into_split();
    
    let client = Arc::new(Mutex::new(Client::new(
        writer, 
        addr,
        server.config.server.name.clone(),
        Arc::clone(&server)
    )));

    let client_id = client.lock().await.id();
    debug!("Created new client with ID {} for {}", client_id, addr);

    // Start the connection handler
    let connection_future = async {
        server.add_client(Arc::clone(&client)).await;
        
        let result = {
            let mut client = client.lock().await;
            client.handle_connection_with_reader(reader).await
        };

        // Cleanup
        {
            let mut client = client.lock().await;
            client.cleanup().await;
        }
        server.remove_client(client_id).await;
        result
    };
    tokio::pin!(connection_future);

    // Run the connection with registration timeout
    let timeout_future = tokio::time::sleep(Duration::from_secs(60));
    tokio::pin!(timeout_future);

    tokio::select! {
        result = &mut connection_future => {
            return result;
        }
        _ = &mut timeout_future => {
            // Get a fresh lock for the registration check
            let is_registered = {
                let client = client.lock().await;
                client.is_registered()
            };
            
            if !is_registered {
                let mut client = client.lock().await;
                client.send_error("Registration timeout").await?;
                return Err(IrcError::Protocol("Registration timeout".into()));
            }
            // If registered, just return the connection future
            return connection_future.await;
        }
    }
}

impl Clone for Server {
    fn clone(&self) -> Self {
        Self {
            config: Arc::clone(&self.config),
            clients: Arc::clone(&self.clients),
            client_map: Arc::clone(&self.client_map),
            channels: Arc::clone(&self.channels),
            database: self.database.clone(),
            nicknames: Arc::clone(&self.nicknames),
            registration_timeouts: Arc::clone(&self.registration_timeouts),
            nickname_map: Arc::clone(&self.nickname_map),
            tx: self.tx.clone(),
            linked_servers: Arc::clone(&self.linked_servers),
        }
    }
}

async fn server_task(rx: mpsc::Receiver<ServerMessage>) {
    let mut clients: HashMap<String, WhoisInfo> = HashMap::new();

    while let Ok(msg) = rx.recv() {
        match msg {
            ServerMessage::WhoisLookup { nickname, respond_to } => {
                let info = clients.get(&nickname.to_lowercase()).cloned();
                respond_to.send(info).ok();
            }
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests; 