#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tokio::net::TcpStream;
    use std::net::SocketAddr;
    use chrono::Utc;
    use crate::server::Server;
    use crate::config::{ServerConfig, KLine};
    use crate::client::Client;
    use tokio::time::{sleep, Duration};
    use std::net::TcpStream as StdTcpStream;

    // Each test gets its own port in the 6900 range
    const PORT_CLIENT_MANAGEMENT: u16 = 6901;
    const PORT_CHANNEL_MANAGEMENT: u16 = 6902;
    const PORT_ACCESS_CONTROL: u16 = 6903;

    // Helper function to create a test config with specified port
    fn test_config(port: u16) -> ServerConfig {
        ServerConfig {
            server: crate::config::Server {
                name: "test.server".to_string(),
                description: "Test Server".to_string(),
                sid: "001".to_string(),
                bind_addr: "127.0.0.1".to_string(),
                port,
            },
            network: crate::config::Network {
                name: "TestNet".to_string(),
                links: vec![],
            },
            limits: crate::config::Limits {
                max_clients: 100,
                max_channels: 50,
            },
            hostmask: None,
            access: crate::config::AccessConfig::default(),
            database: None,
            timeouts: Default::default(),
            links: vec![],
        }
    }

    async fn wait_for_server(addr: &SocketAddr) {
        for _ in 0..50 {  // Try for 5 seconds
            if StdTcpStream::connect(addr).is_ok() {
                return;
            }
            sleep(Duration::from_millis(100)).await;
        }
        panic!("Server failed to start within timeout");
    }

    async fn start_server(server: Arc<Server>, port: u16) {
        let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        
        // Spawn the server accept loop
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let server = Arc::clone(&server);
                tokio::spawn(async move {
                    crate::server::handle_connection(stream, server).await.ok();
                });
            }
        });
    }

    #[tokio::test]
    async fn test_client_management() {
        let server = Arc::new(Server::new(test_config(PORT_CLIENT_MANAGEMENT)).await.unwrap());
        let addr: SocketAddr = format!("127.0.0.1:{}", PORT_CLIENT_MANAGEMENT).parse().unwrap();
        
        // Start the server
        start_server(Arc::clone(&server), PORT_CLIENT_MANAGEMENT).await;
        
        // Wait for server to be ready
        wait_for_server(&addr).await;
        
        // Create test client
        let stream = TcpStream::connect(addr).await.unwrap();
        let client = Arc::new(Mutex::new(Client::new(
            stream.into_split().1,
            addr,
            "test.server".to_string(),
            Arc::clone(&server)
        )));
        
        let client_id = client.lock().await.id();
        
        // Test adding client
        server.add_client(Arc::clone(&client)).await;
        assert!(server.get_client(client_id).await.is_some());
        
        // Test finding client by nick
        {
            let mut client_lock = client.lock().await;
            client_lock.set_nickname("testnick".to_string()).unwrap();
        }
        server.register_nickname("testnick", client_id).await.unwrap();
        let found = server.find_client_by_nick("testnick").await;
        assert!(found.is_some());
        
        // Test removing client
        server.remove_client(client_id).await;
        assert!(server.get_client(client_id).await.is_none());
    }

    #[tokio::test]
    async fn test_channel_management() {
        let server = Arc::new(Server::new(test_config(PORT_CHANNEL_MANAGEMENT)).await.unwrap());
        
        // Test channel creation
        let channel = server.get_or_create_channel("#test").await;
        assert!(server.get_channel("#test").await.is_some());
        
        // Test adding client to channel
        let client_id = 1;
        {
            let mut channel = channel.write().await;
            channel.add_member(client_id);
            assert!(channel.get_members().contains(&client_id));
        }
        
        // Test channel membership check
        assert!(server.check_channel_membership("#test", client_id).await);
        
        // Test getting channel members
        let members = server.get_channel_members("#test").await;
        assert_eq!(members.len(), 1);
        assert!(members.contains(&client_id));
    }

    #[tokio::test]
    async fn test_access_control() {
        let mut config = test_config(PORT_ACCESS_CONTROL);
        config.access.klines.push(KLine {
            mask: "*!*@banned.com".to_string(),
            reason: "Test ban".to_string(),
            set_by: "admin".to_string(),
            duration: 0,
            set_time: Utc::now(),
        });
        
        let server = Arc::new(Server::new(config).await.unwrap());
        let addr: SocketAddr = format!("127.0.0.1:{}", PORT_ACCESS_CONTROL).parse().unwrap();
        
        // Start the server
        start_server(Arc::clone(&server), PORT_ACCESS_CONTROL).await;
        
        // Wait for server to be ready
        wait_for_server(&addr).await;
        
        let stream = TcpStream::connect(addr).await.unwrap();
        
        // Create client without Arc<Mutex<>>
        let mut client = Client::new(
            stream.into_split().1,
            addr,
            "test.server".to_string(),
            Arc::clone(&server)
        );
        
        // Set hostname directly
        client.set_hostname("banned.com".to_string());
        
        // Test K-line check
        assert!(server.check_access(&client).await.is_err());
    }
} 