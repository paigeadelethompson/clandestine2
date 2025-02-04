use std::net::SocketAddr;
use tokio::net::{TcpStream, tcp::{OwnedReadHalf, OwnedWriteHalf}};
use tokio::io::{AsyncWriteExt, BufReader, AsyncBufReadExt};
use crate::error::{IrcError, IrcResult};
use std::collections::HashSet;
use crate::config::ServerConfig;
use crate::server::Server;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

#[cfg(test)]
pub struct TestClient {
    reader: BufReader<OwnedReadHalf>,
    writer: OwnedWriteHalf,
    nickname: String,
    username: String,
    hostname: String,
    capabilities: HashSet<String>,
}

#[cfg(test)]
impl TestClient {
    pub async fn connect(addr: SocketAddr) -> IrcResult<Self> {
        let stream = TcpStream::connect(addr).await?;
        let (read, write) = stream.into_split();
        
        Ok(Self {
            reader: BufReader::new(read),
            writer: write,
            nickname: String::new(),
            username: String::new(),
            hostname: String::new(),
            capabilities: HashSet::new(),
        })
    }

    // Registration methods
    pub async fn register(&mut self, nickname: &str, username: &str, hostname: &str) -> IrcResult<()> {
        self.nickname = nickname.to_string();
        self.username = username.to_string();
        self.hostname = hostname.to_string();

        // Start with CAP negotiation
        self.send_cap_ls().await?;
        let caps = self.handle_cap_ls().await?;

        // Request desired capabilities
        if !caps.is_empty() {
            let cap_list = caps.iter().cloned().collect::<Vec<_>>().join(" ");
            self.send_raw(&format!("CAP REQ :{}", cap_list)).await?;
            self.handle_cap_ack().await?;
            self.capabilities = caps;
        }

        // End capability negotiation
        self.send_cap_end().await?;

        // Send registration sequence
        self.send_nick(nickname).await?;
        self.send_user(username, hostname).await?;
        
        // Wait for registration response
        self.expect_welcome().await?;
        
        Ok(())
    }

    // CAP command handling
    pub async fn send_cap_ls(&mut self) -> IrcResult<()> {
        self.send_raw("CAP LS 302").await
    }

    pub async fn send_cap_end(&mut self) -> IrcResult<()> {
        self.send_raw("CAP END").await
    }

    pub async fn handle_cap_ls(&mut self) -> IrcResult<HashSet<String>> {
        loop {
            let msg = self.read_message().await?;
            if msg.starts_with("CAP * LS") {
                // Parse available capabilities
                let caps = msg.split(':').nth(1).unwrap_or("").split(' ')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .filter(|cap| self.should_request_cap(cap))
                    .collect();
                
                return Ok(caps);
            }
        }
    }

    pub async fn handle_cap_ack(&mut self) -> IrcResult<()> {
        loop {
            let msg = self.read_message().await?;
            if msg.starts_with("CAP * ACK") {
                return Ok(());
            }
            if msg.starts_with("CAP * NAK") {
                return Err(IrcError::Protocol("Capability negotiation failed".to_string()));
            }
        }
    }

    fn should_request_cap(&self, cap: &str) -> bool {
        matches!(cap, 
            "message-tags" | 
            "batch" | 
            "labeled-response" |
            "echo-message" |
            "server-time"
        )
    }

    // Basic IRC commands
    pub async fn send_nick(&mut self, nickname: &str) -> IrcResult<()> {
        self.send_raw(&format!("NICK {}", nickname)).await
    }

    pub async fn send_user(&mut self, username: &str, hostname: &str) -> IrcResult<()> {
        self.send_raw(&format!("USER {} 0 * :{}", username, hostname)).await
    }

    pub async fn join(&mut self, channel: &str) -> IrcResult<()> {
        self.send_raw(&format!("JOIN {}", channel)).await
    }

    pub async fn privmsg(&mut self, target: &str, message: &str) -> IrcResult<()> {
        self.send_raw(&format!("PRIVMSG {} :{}", target, message)).await
    }

    // Low level message handling
    pub async fn send_raw(&mut self, message: &str) -> IrcResult<()> {
        self.writer.write_all(format!("{}\r\n", message).as_bytes()).await?;
        self.writer.flush().await?;
        Ok(())
    }

    pub async fn read_message(&mut self) -> IrcResult<String> {
        let mut line = String::new();
        self.reader.read_line(&mut line).await?;
        Ok(line.trim().to_string())
    }

    // Helper methods
    async fn expect_welcome(&mut self) -> IrcResult<()> {
        loop {
            let msg = self.read_message().await?;
            if msg.contains("001") {  // RPL_WELCOME
                return Ok(());
            }
            if msg.contains("433") {  // ERR_NICKNAMEINUSE
                return Err(IrcError::Protocol("Nickname already in use".to_string()));
            }
        }
    }

    pub fn has_capability(&self, cap: &str) -> bool {
        self.capabilities.contains(cap)
    }

    // Getters
    pub fn nickname(&self) -> &str {
        &self.nickname
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    pub fn hostname(&self) -> &str {
        &self.hostname
    }

    pub async fn set_mode(&mut self, mode: &str) -> IrcResult<()> {
        self.send_raw(&format!("MODE {} {}", self.nickname, mode)).await
    }

    pub async fn expect_mode_change(&mut self, expected_mode: &str) -> IrcResult<()> {
        loop {
            let msg = self.read_message().await?;
            if msg.contains(&format!("MODE {} {}", self.nickname, expected_mode)) {
                return Ok(());
            }
        }
    }

    pub async fn set_topic(&mut self, channel: &str, topic: &str) -> IrcResult<()> {
        self.send_raw(&format!("TOPIC {} :{}", channel, topic)).await
    }

    pub async fn get_topic(&mut self, channel: &str) -> IrcResult<String> {
        self.send_raw(&format!("TOPIC {}", channel)).await?;
        
        loop {
            let msg = self.read_message().await?;
            if msg.contains("332") {  // RPL_TOPIC
                return Ok(msg.split(':').nth(1).unwrap_or("").to_string());
            }
            if msg.contains("331") {  // RPL_NOTOPIC
                return Ok(String::new());
            }
        }
    }

    pub async fn set_channel_mode(&mut self, channel: &str, mode: &str) -> IrcResult<()> {
        self.send_raw(&format!("MODE {} {}", channel, mode)).await?;
        self.expect_mode_response(channel, mode).await
    }

    pub async fn set_channel_ban(&mut self, channel: &str, mask: &str) -> IrcResult<()> {
        self.send_raw(&format!("MODE {} +b {}", channel, mask)).await?;
        self.expect_mode_response(channel, &format!("+b {}", mask)).await
    }

    async fn expect_mode_response(&mut self, channel: &str, mode: &str) -> IrcResult<()> {
        loop {
            let msg = self.read_message().await?;
            if msg.contains(&format!("MODE {} {}", channel, mode)) {
                return Ok(());
            }
            if msg.contains("482") { // ERR_CHANOPRIVSNEEDED
                return Err(IrcError::Protocol("Not channel operator".to_string()));
            }
        }
    }

    pub async fn expect_join(&mut self, channel: &str, nickname: &str) -> IrcResult<()> {
        loop {
            let msg = self.read_message().await?;
            if msg.contains(&format!(":{} JOIN {}", nickname, channel)) {
                return Ok(());
            }
        }
    }
}

// Common test server setup
pub async fn setup_test_server(port: u16) -> (Arc<Server>, SocketAddr) {
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

// Standard test config
#[cfg(test)]
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

// Helper functions
#[cfg(test)]
pub async fn wait_for_server(addr: &SocketAddr) {
    for _ in 0..50 {
        if tokio::net::TcpStream::connect(addr).await.is_ok() {
            return;
        }
        sleep(Duration::from_millis(100)).await;
    }
    panic!("Server failed to start within timeout");
} 