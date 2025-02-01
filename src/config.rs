use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Deserialize)]
pub struct ServerConfig {
    pub server: Server,
    pub network: Network,
    pub limits: Limits,
}

#[derive(Deserialize)]
pub struct Server {
    pub name: String,
    pub description: String,
    pub sid: String, // Server ID for TS6
    pub bind_addr: String,
    pub port: u16,
}

#[derive(Deserialize)]
pub struct Network {
    pub name: String,
    #[serde(default)]
    pub links: Vec<Link>,
}

#[derive(Deserialize)]
pub struct Link {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub send_password: String,
    pub receive_password: String,
    pub sid: String,
}

#[derive(Deserialize)]
pub struct Limits {
    pub max_clients: usize,
    pub max_channels: usize,
}

impl ServerConfig {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let contents = fs::read_to_string(path)?;
        let config: ServerConfig = toml::from_str(&contents)?;
        Ok(config)
    }
} 