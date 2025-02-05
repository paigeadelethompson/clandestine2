use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use chrono::{DateTime, Utc};
use regex;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::channel::Channel;
use crate::client::Client;
use crate::config::{DLine, GLine, ILine, KLine, ServerConfig, ServerLinkConfig};
use crate::database::Database;
use crate::error::{IrcError, IrcResult};
use crate::link::ServerLink;
use crate::server::client::{ServerMessage, WhoisInfo};
use crate::ts6::TS6Message;

mod link;
#[cfg(test)]
mod tests;
mod ping;
mod channel;
mod xline;
mod registration;
mod mask;
mod client;
mod pass;
mod stats;

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

    pub async fn connect_to_server(&self, config: &ServerLinkConfig) -> IrcResult<()> {
        debug!("Connecting to server {} at {}", config.name, config.address);

        // Connect to remote server
        let stream = TcpStream::connect(&config.address).await.map_err(|e| {
            error!("Failed to connect to {}: {}", config.address, e);
            IrcError::Io(e)
        })?;

        // Create server link with the stream
        let server_link = ServerLink::new(
            stream,
            config.name.clone(),
            config.sid.clone(),
            config.description.clone(),
            config.password.clone(),
        );

        // Store server link
        let server_link = Arc::new(Mutex::new(server_link));
        self.linked_servers.write().await.insert(config.name.clone(), Arc::clone(&server_link));

        // Spawn server link handler - no need to pass stream since it's stored in ServerLink
        let server_link_clone = Arc::clone(&server_link);
        tokio::spawn(async move {
            if let Err(e) = server_link_clone.lock().await.handle_connection().await {
                error!("Server link error: {}", e);
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
        Arc::clone(&server),
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
