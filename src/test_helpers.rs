use crate::config::ServerConfig;
use std::net::SocketAddr;
use tokio::time::{sleep, Duration};
use tokio::net::TcpStream;

#[cfg(test)]
pub(crate) mod test {
    use super::*;

    pub fn test_config(port: u16) -> ServerConfig {
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

    pub async fn wait_for_server(addr: &SocketAddr) {
        for _ in 0..50 {  // Try for 5 seconds
            if TcpStream::connect(addr).await.is_ok() {
                return;
            }
            sleep(Duration::from_millis(100)).await;
        }
        panic!("Server failed to start within timeout");
    }
} 