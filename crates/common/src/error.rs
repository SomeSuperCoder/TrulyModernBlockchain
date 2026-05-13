use thiserror::Error;

#[derive(Error, Debug)]
pub enum BlockchainError {
    #[error("Storage error: {0}")]
    Storage(String),
    
    #[error("Consensus error: {0}")]
    Consensus(String),
    
    #[error("Execution error: {0}")]
    Execution(String),
    
    #[error("Invalid state: {0}")]
    InvalidState(String),
    
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    #[error("Cryptography error: {0}")]
    CryptoError(String),
    
    #[error("Invalid validator: {0}")]
    InvalidValidator(String),
}

pub type Result<T> = std::result::Result<T, BlockchainError>;
