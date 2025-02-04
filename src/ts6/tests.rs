#[cfg(test)]
mod parser_tests {
    use crate::ts6::parser::parse_message;

    #[test]
    fn test_parse_basic_message() {
        let msg = parse_message("NICK test 1 :Real Name");
        assert!(msg.is_ok());
        let msg = msg.unwrap();
        assert_eq!(msg.command, "NICK");
        assert_eq!(msg.params, vec!["test", "1", "Real Name"]);
    }

    // ... keep all the other parser unit tests ...
}

#[cfg(test)]
mod integration_tests {
    use std::sync::Arc;
    use crate::server::Server;
    use crate::test_utils::{TestClient, setup_test_server};
    use std::net::SocketAddr;

    const PORT_TS6_SERVER_LINK: u16 = 6931;

    #[tokio::test]
    async fn test_ts6_server_link() {
        let (server1, addr1) = setup_test_server(PORT_TS6_SERVER_LINK).await;
        let (server2, _) = setup_test_server(PORT_TS6_SERVER_LINK + 1).await;

        // Connect server2 to server1
        server2.connect_to_server(&server1.config.links[0]).await.unwrap();

        // Connect a client to server1 and verify it shows up on server2
        let mut client = TestClient::connect(addr1).await.unwrap();
        client.register("test", "user", "test.com").await.unwrap();

        // TODO: Add verification that client appears on server2
    }

    // Add more TS6 tests...
} 