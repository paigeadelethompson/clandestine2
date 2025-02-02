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
            if let Ok(_) = TcpStream::connect(addr).await {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
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

    #[tokio::test]
    async fn test_server_nickname_management() {
        let port = 6904;
        let server = Arc::new(Server::new(test_config(port)).await.unwrap());
        let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
        
        start_server(Arc::clone(&server), port).await;
        wait_for_server(&addr).await;
        
        // Test nickname registration
        server.register_nickname("testnick", 1).await.unwrap();
        assert!(server.is_nickname_registered("testnick").await);
        
        // Test duplicate nickname
        assert!(server.register_nickname("testnick", 2).await.is_err());
        
        // Test nickname release
        server.release_nickname("testnick").await;
        assert!(!server.is_nickname_registered("testnick").await);
        
        // Test registering released nickname
        assert!(server.register_nickname("testnick", 2).await.is_ok());
    }

    #[tokio::test]
    async fn test_server_channel_limits() {
        let port = 6905;
        let mut config = test_config(port);
        config.limits.max_channels = 2;
        let server = Arc::new(Server::new(config).await.unwrap());
        
        // Test channel creation within limits
        let channel1 = server.get_or_create_channel("#test1").await;
        let channel2 = server.get_or_create_channel("#test2").await;
        assert!(channel1.read().await.name == "#test1");
        assert!(channel2.read().await.name == "#test2");
        
        // Test channel limit enforcement
        let channel3 = server.get_or_create_channel("#test3").await;
        assert!(channel3.read().await.name == "#test3");
        assert_eq!(server.get_channel_count().await, 2);
    }

    #[tokio::test]
    async fn test_server_client_limits() {
        let port = 6906;
        let mut config = test_config(port);
        config.limits.max_clients = 2;
        let server = Arc::new(Server::new(config).await.unwrap());
        let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
        
        start_server(Arc::clone(&server), port).await;
        wait_for_server(&addr).await;
        
        // Create test clients
        let stream1 = TcpStream::connect(addr).await.unwrap();
        let client1 = Arc::new(Mutex::new(Client::new(
            stream1.into_split().1,
            addr,
            "test.server".to_string(),
            Arc::clone(&server)
        )));

        let stream2 = TcpStream::connect(addr).await.unwrap();
        let client2 = Arc::new(Mutex::new(Client::new(
            stream2.into_split().1,
            addr,
            "test.server".to_string(),
            Arc::clone(&server)
        )));
        
        // Test adding clients within limit
        server.add_client(Arc::clone(&client1)).await;
        server.add_client(Arc::clone(&client2)).await;
        assert_eq!(server.get_client_count().await, 2);
        
        // Test client limit enforcement
        let stream3 = TcpStream::connect(addr).await.unwrap();
        let client3 = Arc::new(Mutex::new(Client::new(
            stream3.into_split().1,
            addr,
            "test.server".to_string(),
            Arc::clone(&server)
        )));
        
        server.add_client(Arc::clone(&client3)).await;
        assert_eq!(server.get_client_count().await, 2);
    }

    #[tokio::test]
    async fn test_server_kline_management() {
        let port = 6907;
        let server = Arc::new(Server::new(test_config(port)).await.unwrap());
        
        // Start server in background task
        let server_clone = Arc::clone(&server);
        tokio::spawn(async move {
            server_clone.run().await.unwrap();
        });

        // Wait for server to start
        let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
        wait_for_server(&addr).await;
        
        // Add K-line
        let kline = KLine {
            mask: "*!*@banned.com".to_string(),
            reason: "Test ban".to_string(),
            set_by: "admin".to_string(),
            duration: 0,
            set_time: Utc::now(),
        };
        server.add_kline(kline).await.unwrap();
        
        // Try to connect - should fail due to K-line
        let stream = match TcpStream::connect(&addr).await {
            Ok(stream) => stream,
            Err(e) => {
                panic!("Failed to connect to test server: {}", e);
            }
        };

        let mut client = Client::new(
            stream.into_split().1,
            addr,
            "test.server".to_string(),
            Arc::clone(&server)
        );
        client.set_hostname("banned.com".to_string());
        
        assert!(server.check_access(&client).await.is_err());
        
        // Remove K-line and try again
        server.remove_kline("*!*@banned.com".to_string()).await.unwrap();
        assert!(server.check_access(&client).await.is_ok());
    }
} 