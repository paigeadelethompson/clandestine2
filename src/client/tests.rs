#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tokio::net::TcpStream;
    use std::net::SocketAddr;
    use crate::server::Server;
    use crate::config::ServerConfig;
    use crate::client::Client;
    use crate::ts6::TS6Message;
    use crate::ircv3::Capability;

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
    async fn test_capability_negotiation() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let server = Arc::new(Server::new(test_config()).await.unwrap());
        let stream = TcpStream::connect(addr).await.unwrap();
        let client = Arc::new(Mutex::new(Client::new(
            stream.into_split().1,
            addr,
            "test.server".to_string(),
            server
        )));

        // Test CAP LS
        let cap_ls = TS6Message::new("CAP".to_string(), vec!["LS".to_string()]);
        client.lock().await.handle_cap(cap_ls).await.unwrap();
        assert!(client.lock().await.cap_negotiating);

        // Test CAP REQ
        let cap_req = TS6Message::new("CAP".to_string(), 
            vec!["REQ".to_string(), "multi-prefix extended-join".to_string()]);
        client.lock().await.handle_cap(cap_req).await.unwrap();
        
        let caps = client.lock().await.enabled_capabilities.clone();
        assert!(caps.contains(&Capability::MultiPrefix));
        assert!(caps.contains(&Capability::ExtendedJoin));

        // Test CAP END
        let cap_end = TS6Message::new("CAP".to_string(), vec!["END".to_string()]);
        client.lock().await.handle_cap(cap_end).await.unwrap();
        assert!(!client.lock().await.cap_negotiating);
    }

    #[tokio::test]
    async fn test_capability_checks() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let server = Arc::new(Server::new(test_config()).await.unwrap());
        let stream = TcpStream::connect(addr).await.unwrap();
        let client = Arc::new(Mutex::new(Client::new(
            stream.into_split().1,
            addr,
            "test.server".to_string(),
            server
        )));

        {
            let mut client_lock = client.lock().await;
            client_lock.enabled_capabilities.insert(Capability::MultiPrefix);
            assert!(client_lock.has_capability(&Capability::MultiPrefix));
            assert!(!client_lock.has_capability(&Capability::ExtendedJoin));
        }
    }
} 