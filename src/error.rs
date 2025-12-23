use thiserror::Error;

#[derive(Error, Debug)]
pub enum CardError {
    #[error("Card '{0}' not found")]
    NotFound(String),

    #[error("Card '{0}' already exists")]
    AlreadyExists(String),

    #[error("Invalid card name: {0}")]
    InvalidName(String),

    #[error("Invalid status: {0}")]
    InvalidStatus(String),

    #[error("Storage error: {0}")]
    Storage(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] toml::ser::Error),

    #[error("Deserialization error: {0}")]
    Deserialization(#[from] toml::de::Error),
}

pub type Result<T> = std::result::Result<T, CardError>;
