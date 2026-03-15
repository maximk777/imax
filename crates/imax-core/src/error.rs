use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Identity error: {0}")]
    Identity(String),

    #[error("Crypto error: {0}")]
    Crypto(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Chat error: {0}")]
    Chat(String),
}

pub type Result<T> = std::result::Result<T, Error>;
