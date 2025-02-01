mod config;
mod server;
mod client;
mod channel;
mod ts6;
mod error;
mod cli;
mod ircv3;
mod database;

use std::error::Error;
use std::fs;
use crate::config::ServerConfig;
use crate::server::Server;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
use clap::Parser;
use crate::cli::{Cli, generate_example_config};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    // Handle config generation
    if cli.generate_config {
        let example_config = generate_example_config();
        println!("{}", example_config);
        return Ok(());
    }

    // Initialize logging
    let log_level = match cli.log_level.to_lowercase().as_str() {
        "error" => Level::ERROR,
        "warn" => Level::WARN,
        "info" => Level::INFO,
        "debug" => Level::DEBUG,
        "trace" => Level::TRACE,
        _ => Level::INFO,
    };

    let subscriber = FmtSubscriber::builder()
        .with_max_level(log_level)
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_target(false)
        .with_env_filter("ircd_rs=debug")
        .init();
    
    info!("Starting IRCd...");
    
    let config = ServerConfig::load(&cli.config).map_err(|e| {
        tracing::error!("Failed to load configuration from {:?}: {}", cli.config, e);
        e
    })?;
    
    info!("Configuration loaded successfully");
    let server = Server::new(config).await?;
    
    info!("Starting server...");
    server.run().await?;
    
    Ok(())
} 