use serde::{Deserialize, Serialize};

/// 32-byte file identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileID(pub [u8; 32]);

impl Default for FileID {
    fn default() -> Self {
        Self([0u8; 32])
    }
}

impl FileID {
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// 32-byte state hash
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StateHash(pub [u8; 32]);

/// 32-byte transaction identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TransactionID(pub [u8; 32]);

/// 32-byte block hash
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockHash(pub [u8; 32]);

/// Validator identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ValidatorID(pub [u8; 32]);
