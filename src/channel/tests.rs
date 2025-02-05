use std::net::SocketAddr;
use std::sync::Arc;

use crate::server::Server;
use crate::test_helpers::test::{test_config, wait_for_server};
use crate::test_utils::TestClient;

use super::*;

#[cfg(test)]
mod tests {
    use super::*;

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

        // Connect and register first client
        let mut client1 = TestClient::connect(addr).await.unwrap();
        let result = client1.register("nick1", "user1", "test.com").await;
        assert!(result.is_ok(), "First client registration failed: {:?}", result);

        // Connect and register second client
        let mut client2 = TestClient::connect(addr).await.unwrap();
        let result = client2.register("nick2", "user2", "test.com").await;
        assert!(result.is_ok(), "Second client registration failed: {:?}", result);

        // First client creates channel
        client1.join("#test").await.unwrap();

        // Verify client1 is in channel and has operator status
        let mut found_nick1 = false;
        loop {
            let msg = client1.read_message().await.unwrap();
            if msg.contains("353") && msg.contains("@nick1") { // RPL_NAMREPLY with @ for operator
                found_nick1 = true;
            }
            if msg.contains("366") { // RPL_ENDOFNAMES
                break;
            }
        }
        assert!(found_nick1, "First client should be in channel with operator status");

        // Second client joins
        client2.join("#test").await.unwrap();

        // Verify both clients are in channel
        let mut found_nick1 = false;
        let mut found_nick2 = false;
        loop {
            let msg = client2.read_message().await.unwrap();
            if msg.contains("353") { // RPL_NAMREPLY
                if msg.contains("@nick1") {
                    found_nick1 = true;
                }
                if msg.contains("nick2") {
                    found_nick2 = true;
                }
            }
            if msg.contains("366") { // RPL_ENDOFNAMES
                break;
            }
        }
        assert!(found_nick1, "First client should still be in channel");
        assert!(found_nick2, "Second client should be in channel");
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