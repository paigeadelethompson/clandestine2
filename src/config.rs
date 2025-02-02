use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::net::IpAddr;
use chrono::{DateTime, Utc};

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub server: Server,
    pub network: Network,
    pub limits: Limits,
    pub hostmask: Option<HostmaskConfig>,
    pub access: AccessConfig,
    pub database: Option<DatabaseConfig>,
    #[serde(default)]
    pub timeouts: Timeouts,
    #[serde(default)]
    pub links: Vec<ServerLinkConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Server {
    pub name: String,
    pub description: String,
    pub sid: String, // Server ID for TS6
    pub bind_addr: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Network {
    pub name: String,
    #[serde(default)]
    pub links: Vec<Link>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Link {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub send_password: String,
    pub receive_password: String,
    pub sid: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Limits {
    pub max_clients: usize,
    pub max_channels: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub struct HostmaskConfig {
    pub enabled: bool,
    pub format: String,  // e.g. "user/{user}/host/{host}"
    pub prefix: String,  // e.g. "cloaked"
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct AccessConfig {
    #[serde(default)]
    pub klines: Vec<KLine>,
    #[serde(default)]
    pub dlines: Vec<DLine>,
    #[serde(default)]
    pub glines: Vec<GLine>,
    #[serde(default)]
    pub ilines: Vec<ILine>,
    #[serde(default)]
    pub olines: Vec<OLine>,
    #[serde(default)]
    pub ulines: Vec<ULine>,
    #[serde(default)]
    pub alines: Vec<ALine>,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct KLine {
    pub mask: String,           // nick!user@host mask
    pub reason: String,
    pub set_by: String,
    #[serde(default = "default_duration")]
    pub duration: i64,          // Duration in seconds, 0 for permanent
    #[serde(default = "Utc::now")]
    pub set_time: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct DLine {
    pub ip: IpAddr,            // IP or network to ban
    pub reason: String,
    pub set_by: String,
    #[serde(default = "default_duration")]
    pub duration: i64,
    #[serde(default = "Utc::now")]
    pub set_time: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct GLine {
    pub mask: String,          // Global ban mask
    pub reason: String,
    pub set_by: String,
    #[serde(default = "default_duration")]
    pub duration: i64,
    #[serde(default = "Utc::now")]
    pub set_time: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct ILine {
    pub mask: String,          // Allow connection mask
    pub password: Option<String>,
    pub class: String,         // Connection class
    pub max_connections: u32,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct OLine {
    pub mask: String,          // Operator access mask
    pub password: String,      // Encrypted password
    pub flags: Vec<String>,    // Operator privileges
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct ULine {
    pub server: String,        // Trusted server name
    pub flags: Vec<String>,    // Server privileges
}

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct ALine {
    pub mask: String,          // Allow auth mask
    pub password: String,      // Authentication password
    pub class: String,         // Auth class
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub path: String,     // Path to the database file
    pub persist_lines: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Timeouts {
    #[serde(default = "default_ping_interval")]
    pub ping_interval: u64,
    #[serde(default = "default_ping_timeout")]
    pub ping_timeout: u64,
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            ping_interval: default_ping_interval(),
            ping_timeout: default_ping_timeout(),
        }
    }
}

fn default_duration() -> i64 {
    0 // Permanent by default
}

fn default_ping_interval() -> u64 {
    16 // Default ping interval in seconds
}

fn default_ping_timeout() -> u64 {
    128 // Default ping timeout in seconds
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerLinkConfig {
    pub name: String,
    pub sid: String,
    pub description: String,
    pub password: String,
    pub address: String,  // IP:Port for connecting
    pub autoconnect: bool,
    pub ssl: bool,
}

impl ServerConfig {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let contents = fs::read_to_string(path)?;
        let config: ServerConfig = toml::from_str(&contents)?;
        Ok(config)
    }
} 