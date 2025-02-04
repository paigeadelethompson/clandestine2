use super::*;
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::test_utils::TestClient;
use crate::server::Server;
use crate::config::ServerConfig;
use std::net::SocketAddr;
use crate::test_helpers::test::{test_config, wait_for_server};

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    const PORT_CHANNEL_JOIN: u16 = 6921;
    const PORT_CHANNEL_PART: u16 = 6922;
    const PORT_CHANNEL_TOPIC: u16 = 6923;
    const PORT_CHANNEL_MODES: u16 = 6924;
    const PORT_CHANNEL_BANS: u16 = 6925;

    async fn setup_test_server(port: u16) -> (Arc<Server>, SocketAddr) {
        let server = Arc::new(Server::new(test_config(port)).await.unwrap());
        
        // Start server properly
        let server_clone = Arc::clone(&server);
        tokio::spawn(async move {
            server_clone.run().await.unwrap();
        });

        let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
        wait_for_server(&addr).await;
        
        (server, addr)
    }

    #[tokio::test]
    async fn test_channel_join() {
        let (_server, addr) = setup_test_server(PORT_CHANNEL_JOIN).await;
        
        // Connect and register two clients
        let mut client1 = TestClient::connect(addr).await.unwrap();
        client1.register("nick1", "user1", "test.com").await.unwrap();
        
        let mut client2 = TestClient::connect(addr).await.unwrap();
        client2.register("nick2", "user2", "test.com").await.unwrap();

        // First client creates channel
        client1.join("#test").await.unwrap();
        
        // Second client joins
        client2.join("#test").await.unwrap();
        
        // TODO: Add verification of channel membership through NAMES or WHO
    }

    #[tokio::test]
    async fn test_channel_topic() {
        let (server, addr) = setup_test_server(PORT_CHANNEL_TOPIC).await;
        
        // Connect and register client
        let mut client = TestClient::connect(addr).await.unwrap();
        client.register("nick1", "user1", "test.com").await.unwrap();

        // Join channel and set topic
        client.join("#test").await.unwrap();
        client.send_raw("TOPIC #test :New channel topic").await.unwrap();
        
        // Verify topic was set
        let response = client.read_message().await.unwrap();
        assert!(response.contains("New channel topic"));
    }

    #[tokio::test]
    async fn test_channel_modes() {
        let (server, addr) = setup_test_server(PORT_CHANNEL_MODES).await;
        
        // Connect and register op client
        let mut op_client = TestClient::connect(addr).await.unwrap();
        op_client.register("op", "user1", "test.com").await.unwrap();

        // Create channel and get op status
        op_client.join("#test").await.unwrap();
        
        // Set channel modes
        op_client.set_channel_mode("#test", "+n").await.unwrap();
        op_client.set_channel_mode("#test", "+t").await.unwrap();
        
        // Connect regular client
        let mut regular_client = TestClient::connect(addr).await.unwrap();
        regular_client.register("user", "user2", "test.com").await.unwrap();
        
        // Join should work
        regular_client.join("#test").await.unwrap();
        
        // Topic change should fail for regular user due to +t
        let result = regular_client.set_topic("#test", "New Topic").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_channel_bans() {
        let (server, addr) = setup_test_server(PORT_CHANNEL_BANS).await;
        
        // Connect op client
        let mut op_client = TestClient::connect(addr).await.unwrap();
        op_client.register("op", "user1", "test.com").await.unwrap();
        
        // Create channel
        op_client.join("#test").await.unwrap();
        
        // Set ban
        op_client.set_channel_ban("#test", "*!*@banned.com").await.unwrap();
        
        // Try to join with banned host
        let mut banned_client = TestClient::connect(addr).await.unwrap();
        let result = banned_client.register("banned", "user", "banned.com").await;
        assert!(result.is_err());
    }

    // Add more tests for modes, bans, etc
} 