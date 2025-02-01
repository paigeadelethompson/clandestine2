use thiserror::Error;

#[derive(Error, Debug)]
pub enum IrcError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Client error: {0}")]
    Client(String),

    #[error("Channel error: {0}")]
    Channel(String),

    #[error("Server link error: {0}")]
    ServerLink(String),

    #[error("Parse error: {0}")]
    Parse(String),
}

pub type IrcResult<T> = Result<T, IrcError>; 