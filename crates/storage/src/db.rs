// RocksDB wrapper placeholder
// Will be implemented in Phase 1

use blockchain_common::Result;

pub struct Database {
    // TODO: Implement RocksDB wrapper
}

impl Database {
    pub fn new(_path: &str) -> Result<Self> {
        Ok(Self {})
    }
}
