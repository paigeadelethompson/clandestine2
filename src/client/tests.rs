#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::net::SocketAddr;
    use std::sync::Arc;

    use tokio::time::{Duration, sleep};

    use crate::config::ServerConfig;
    use crate::server::Server;
    use crate::test_utils::TestClient;

    // Each test gets its own port in the 6910 range
    const PORT_CAPABILITY_NEGOTIATION: u16 = 6911;
    const PORT_CAPABILITY_CHECKS: u16 = 6912;
    const PORT_CLIENT_REGISTRATION: u16 = 6913;
    const PORT_CLIENT_MODES: u16 = 6914;
    const PORT_CLIENT_PING: u16 = 6915;

    // Helper function to create a test config
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
            if std::net::TcpStream::connect(addr).is_ok() {
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
    async fn test_capability_negotiation() {
        let server = Arc::new(Server::new(test_config(PORT_CAPABILITY_NEGOTIATION)).await.unwrap());

        // Start server
        let server_clone = Arc::clone(&server);
        tokio::spawn(async move {
            server_clone.run().await.unwrap();
        });

        let addr: SocketAddr = format!("127.0.0.1:{}", PORT_CAPABILITY_NEGOTIATION).parse().unwrap();
        wait_for_server(&addr).await;

        // Connect and test capabilities
        let mut client = TestClient::connect(addr).await.unwrap();

        // Test CAP LS
        client.send_cap_ls().await.unwrap();
        let caps = client.handle_cap_ls().await.unwrap();
        assert!(!caps.is_empty());

        // Test CAP REQ
        let cap_list = caps.iter().cloned().collect::<Vec<_>>().join(" ");
        client.send_raw(&format!("CAP REQ :{}", cap_list)).await.unwrap();
        client.handle_cap_ack().await.unwrap();

        // Test CAP END
        client.send_cap_end().await.unwrap();
    }

    #[tokio::test]
    async fn test_capability_checks() {
        let server = Arc::new(Server::new(test_config(PORT_CAPABILITY_CHECKS)).await.unwrap());

        // Start server properly
        let server_clone = Arc::clone(&server);
        tokio::spawn(async move {
            server_clone.run().await.unwrap();
        });

        let addr: SocketAddr = format!("127.0.0.1:{}", PORT_CAPABILITY_CHECKS).parse().unwrap();
        wait_for_server(&addr).await;

        // Connect and register with specific capabilities
        let mut client = TestClient::connect(addr).await.unwrap();

        // Request only multi-prefix capability
        client.send_cap_ls().await.unwrap();
        let mut caps = HashSet::new();
        caps.insert("multi-prefix".to_string());

        let cap_list = caps.iter().cloned().collect::<Vec<_>>().join(" ");
        client.send_raw(&format!("CAP REQ :{}", cap_list)).await.unwrap();
        client.handle_cap_ack().await.unwrap();
        client.send_cap_end().await.unwrap();

        // Verify we have multi-prefix but not extended-join
        assert!(client.has_capability("multi-prefix"));
        assert!(!client.has_capability("extended-join"));
    }

    #[tokio::test]
    async fn test_client_registration() {
        let server = Arc::new(Server::new(test_config(PORT_CLIENT_REGISTRATION)).await.unwrap());

        // Start server properly
        let server_clone = Arc::clone(&server);
        tokio::spawn(async move {
            server_clone.run().await.unwrap();
        });

        let addr: SocketAddr = format!("127.0.0.1:{}", PORT_CLIENT_REGISTRATION).parse().unwrap();
        wait_for_server(&addr).await;

        // Test successful registration
        let mut client1 = TestClient::connect(addr).await.unwrap();
        let result = client1.register("testnick", "testuser", "test.com").await;
        assert!(result.is_ok());
        assert_eq!(client1.nickname(), "testnick");
        assert_eq!(client1.username(), "testuser");

        // Test registration with taken nickname
        let mut client2 = TestClient::connect(addr).await.unwrap();
        let result = client2.register("testnick", "otheruser", "test.com").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_client_modes() {
        let server = Arc::new(Server::new(test_config(PORT_CLIENT_MODES)).await.unwrap());

        // Start server properly
        let server_clone = Arc::clone(&server);
        tokio::spawn(async move {
            server_clone.run().await.unwrap();
        });

        let addr: SocketAddr = format!("127.0.0.1:{}", PORT_CLIENT_MODES).parse().unwrap();
        wait_for_server(&addr).await;

        // Connect and register client
        let mut client = TestClient::connect(addr).await.unwrap();
        client.register("testnick", "testuser", "test.com").await.unwrap();

        // Test setting user mode
        client.send_raw("MODE testnick +i").await.unwrap();
        let response = client.read_message().await.unwrap();
        assert!(response.contains("+i"));

        // Test removing user mode
        client.send_raw("MODE testnick -i").await.unwrap();
        let response = client.read_message().await.unwrap();
        assert!(response.contains("-i"));
    }

    #[tokio::test]
    async fn test_client_ping_pong() {
        let server = Arc::new(Server::new(test_config(PORT_CLIENT_PING)).await.unwrap());

        // Start server properly
        let server_clone = Arc::clone(&server);
        tokio::spawn(async move {
            server_clone.run().await.unwrap();
        });

        let addr: SocketAddr = format!("127.0.0.1:{}", PORT_CLIENT_PING).parse().unwrap();
        wait_for_server(&addr).await;

        // Connect and register client
        let mut client = TestClient::connect(addr).await.unwrap();
        client.register("testnick", "testuser", "test.com").await.unwrap();

        // Test PING response
        client.send_raw("PING :test.server").await.unwrap();
        let response = client.read_message().await.unwrap();
        assert!(response.starts_with("PONG"));
    }
} 