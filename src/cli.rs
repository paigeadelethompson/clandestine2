use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Path to the configuration file
    #[arg(short, long, default_value = "config.toml")]
    pub config: PathBuf,

    /// Generate an example configuration file and exit
    #[arg(short, long)]
    pub generate_config: bool,

    /// Set the log level (error, warn, info, debug, trace)
    #[arg(short, long, default_value = "info")]
    pub log_level: String,
}

pub fn generate_example_config() -> String {
    r#"# IRCd-rs Example Configuration

[server]
name = "irc.example.com"
description = "My IRC Server"
sid = "001"                # Server ID for TS6 protocol
bind_addr = "0.0.0.0"     # Address to bind to
port = 6667               # Port to listen on

[network]
name = "ExampleNet"       # Network name

# Optional server links configuration
# [[network.links]]
# name = "irc2.example.com" # Name of the linked server
# host = "irc2.example.com" # Hostname or IP of the linked server
# port = 6667              # Port of the linked server
# send_password = "linkpass1"    # Password we send to them
# receive_password = "linkpass2" # Password we expect from them
# sid = "002"              # Their Server ID

[limits]
max_clients = 1000       # Maximum number of clients
max_channels = 500       # Maximum number of channels
"#.to_string()
} 