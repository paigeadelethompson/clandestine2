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

    // Helper function to create a test config
    fn test_config() -> ServerConfig {
        ServerConfig {
            server: crate::config::Server {
                name: "test.server".to_string(),
                description: "Test Server".to_string(),
                sid: "001".to_string(),
                bind_addr: "127.0.0.1".to_string(),
                port: 6667,
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

    #[tokio::test]
    async fn test_client_management() {
        let server = Arc::new(Server::new(test_config()).await.unwrap());
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        
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
        let server = Arc::new(Server::new(test_config()).await.unwrap());
        
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
        let mut config = test_config();
        config.access.klines.push(KLine {
            mask: "*!*@banned.com".to_string(),
            reason: "Test ban".to_string(),
            set_by: "admin".to_string(),
            duration: 0,
            set_time: Utc::now(),
        });
        
        let server = Arc::new(Server::new(config).await.unwrap());
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
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