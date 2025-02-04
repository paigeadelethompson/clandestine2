#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::sync::Arc;

    use chrono::Utc;
    use tokio::net::TcpStream;
    use tokio::time::Duration;

    use crate::config::KLine;
    use crate::server::Server;
    use crate::test_utils::{setup_test_server, test_config};
    use crate::test_utils::TestClient;

    // Each test gets its own port
    const PORT_CLIENT_MANAGEMENT: u16 = 6901;
    const PORT_CHANNEL_MANAGEMENT: u16 = 6902;
    const PORT_ACCESS_CONTROL: u16 = 6903;
    const PORT_NICKNAME: u16 = 6904;
    const PORT_CHANNEL_LIMITS: u16 = 6905;
    const PORT_CLIENT_LIMITS: u16 = 6906;
    const PORT_KLINE: u16 = 6907;
    const PORT_CAPABILITIES: u16 = 6908;

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
        let (_server, addr) = setup_test_server(PORT_CLIENT_MANAGEMENT).await;

        // Connect and register first client
        let mut client1 = TestClient::connect(addr).await.unwrap();
        let result = client1.register("testnick", "user", "test.com").await;
        assert!(result.is_ok());

        // Try to register second client with same nickname - should fail
        let mut client2 = TestClient::connect(addr).await.unwrap();
        let result = client2.register("testnick", "user2", "test2.com").await;
        assert!(result.is_err());

        // Register second client with different nickname
        let result = client2.register("othernick", "user2", "test2.com").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_channel_management() {
        let (_server, addr) = setup_test_server(PORT_CHANNEL_MANAGEMENT).await;

        // Connect and register two test clients
        let mut client1 = TestClient::connect(addr).await.unwrap();
        client1.register("nick1", "user1", "test.com").await.unwrap();

        let mut client2 = TestClient::connect(addr).await.unwrap();
        client2.register("nick2", "user2", "test.com").await.unwrap();

        // Test channel operations
        client1.join("#test").await.unwrap();

        // Second client joins same channel
        client2.join("#test").await.unwrap();

        // TODO: Add tests for channel modes, topics, etc once we add those methods to TestClient
    }

    #[tokio::test]
    async fn test_access_control() {
        // Create config with K-line
        let mut config = test_config(PORT_ACCESS_CONTROL);
        config.access.klines.push(KLine {
            mask: "*.evil.com".to_string(),
            reason: "Bad host".to_string(),
            set_by: "admin".to_string(),
            duration: 0,
            set_time: Utc::now(),
        });

        let server = Arc::new(Server::new(config).await.unwrap());

        // Start server and wait for it to be ready
        let server_handle = {
            let server = Arc::clone(&server);
            tokio::spawn(async move {
                server.run().await.unwrap();
            })
        };

        // Give server time to start listening
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Try connecting with banned hostname - should fail
        let addr = SocketAddr::from(([127, 0, 0, 1], PORT_ACCESS_CONTROL));
        let mut client = TestClient::connect(addr).await.unwrap();
        let result = client.register("nick", "user", "evil.com").await;
        assert!(result.is_err());

        // Clean up
        server_handle.abort();
    }

    #[tokio::test]
    async fn test_server_nickname_management() {
        let (_server, addr) = setup_test_server(PORT_NICKNAME).await;

        // Register first client with nickname
        let mut client1 = TestClient::connect(addr).await.unwrap();
        let result = client1.register("testnick", "user1", "test.com").await;
        assert!(result.is_ok());

        // Try to register second client with same nick - should fail
        let mut client2 = TestClient::connect(addr).await.unwrap();
        let result = client2.register("testnick", "user2", "test.com").await;
        assert!(result.is_err());

        // First client disconnects, freeing the nickname
        drop(client1);

        // Now second client should be able to register with that nick
        let result = client2.register("testnick", "user2", "test.com").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_server_kline_management() {
        let (server, addr) = setup_test_server(PORT_KLINE).await;

        // Add K-line
        let kline = KLine {
            mask: "*!*@banned.com".to_string(),
            reason: "Test ban".to_string(),
            set_by: "admin".to_string(),
            duration: 0,
            set_time: Utc::now(),
        };
        server.add_kline(kline).await.unwrap();

        // Test K-line check
        let test_host = "*!*@banned.com".to_string();
        assert!(server.is_host_klined(&test_host).await);

        // Remove K-line
        server.remove_kline("*!*@banned.com".to_string()).await.unwrap();
        assert!(!server.is_host_klined(&test_host).await);
    }

    #[tokio::test]
    async fn test_client_capabilities() {
        let (_server, addr) = setup_test_server(PORT_CAPABILITIES).await;

        // Connect and register with CAP negotiation
        let mut test_client = TestClient::connect(addr).await.unwrap();
        let result = test_client.register("test", "user", "test.com").await;
        assert!(result.is_ok());

        // Verify that we got the capabilities we wanted
        assert!(test_client.has_capability("message-tags"));
        assert!(test_client.has_capability("server-time"));
    }
} 