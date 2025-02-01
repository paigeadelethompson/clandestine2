use crate::channel::Channel;
use crate::config::ServerConfig;
use crate::error::{IrcError, IrcResult};
use crate::client::Client;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

pub struct Server {
    config: Arc<ServerConfig>,
    clients: Arc<RwLock<Vec<ClientId>>>,
    channels: Arc<RwLock<HashMap<String, Channel>>>,
}

type ClientId = u32;

impl Server {
    pub fn new(config: ServerConfig) -> Self {
        info!("Initializing server with configuration");
        Self {
            config: Arc::new(config),
            clients: Arc::new(RwLock::new(Vec::new())),
            channels: Arc::new(RwLock::new(HashMap::new())),
        }
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
}

async fn handle_connection(socket: TcpStream, server: Arc<Server>) -> IrcResult<()> {
    let addr = socket.peer_addr().map_err(|e| {
        error!("Failed to get peer address: {}", e);
        IrcError::Io(e)
    })?;

    let mut client = Client::new(socket, addr, server.config.server.name.clone());
    debug!("Created new client instance for {}", addr);

    // Add client to server's client list
    {
        let mut clients = server.clients.write().await;
        clients.push(client.id);
        debug!("Added client {} to server", client.id);
    }

    // Handle the client connection
    let result = client.handle_connection().await;

    // Clean up when client disconnects
    {
        let mut clients = server.clients.write().await;
        if let Some(pos) = clients.iter().position(|&id| id == client.id) {
            clients.swap_remove(pos);
            info!("Removed client {} from server", client.id);
        }
    }

    result
}

impl Clone for Server {
    fn clone(&self) -> Self {
        Self {
            config: Arc::clone(&self.config),
            clients: Arc::clone(&self.clients),
            channels: Arc::clone(&self.channels),
        }
    }
} 