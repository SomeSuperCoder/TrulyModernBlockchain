// RocksDB wrapper with column families and error handling
use blockchain_common::{BlockchainError, Result};
use rocksdb::{ColumnFamilyDescriptor, Options, DB};
use std::path::Path;
use std::sync::Arc;

/// Column family names for different data types
pub const CF_FILES: &str = "files";
pub const CF_MERKLE: &str = "merkle";
pub const CF_METADATA: &str = "metadata";

/// Database wrapper around RocksDB with column families
pub struct Database {
    db: Arc<DB>,
}

impl Database {
    /// Create a new database instance at the specified path
    /// Sets up column families for files, merkle tree, and metadata
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);
        
        // Configure performance options
        db_opts.set_max_open_files(10000);
        db_opts.set_use_fsync(false);
        db_opts.set_bytes_per_sync(1048576);
        db_opts.set_keep_log_file_num(10);
        db_opts.set_max_background_jobs(4);
        
        // Define column families
        let cf_files = ColumnFamilyDescriptor::new(CF_FILES, Options::default());
        let cf_merkle = ColumnFamilyDescriptor::new(CF_MERKLE, Options::default());
        let cf_metadata = ColumnFamilyDescriptor::new(CF_METADATA, Options::default());
        
        let db = DB::open_cf_descriptors(&db_opts, path, vec![cf_files, cf_merkle, cf_metadata])
            .map_err(|e| BlockchainError::Storage(format!("Failed to open database: {}", e)))?;
        
        Ok(Self {
            db: Arc::new(db),
        })
    }
    
    /// Get a value from the specified column family
    pub fn get(&self, cf: &str, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let cf_handle = self.db
            .cf_handle(cf)
            .ok_or_else(|| BlockchainError::Storage(format!("Column family {} not found", cf)))?;
        
        self.db
            .get_cf(&cf_handle, key)
            .map_err(|e| BlockchainError::Storage(format!("Failed to get key: {}", e)))
    }
    
    /// Put a value into the specified column family
    pub fn put(&self, cf: &str, key: &[u8], value: &[u8]) -> Result<()> {
        let cf_handle = self.db
            .cf_handle(cf)
            .ok_or_else(|| BlockchainError::Storage(format!("Column family {} not found", cf)))?;
        
        self.db
            .put_cf(&cf_handle, key, value)
            .map_err(|e| BlockchainError::Storage(format!("Failed to put key: {}", e)))
    }
    
    /// Delete a value from the specified column family
    pub fn delete(&self, cf: &str, key: &[u8]) -> Result<()> {
        let cf_handle = self.db
            .cf_handle(cf)
            .ok_or_else(|| BlockchainError::Storage(format!("Column family {} not found", cf)))?;
        
        self.db
            .delete_cf(&cf_handle, key)
            .map_err(|e| BlockchainError::Storage(format!("Failed to delete key: {}", e)))
    }
    
    /// Check if a key exists in the specified column family
    pub fn exists(&self, cf: &str, key: &[u8]) -> Result<bool> {
        Ok(self.get(cf, key)?.is_some())
    }
    
    /// Get the underlying RocksDB instance (for advanced operations)
    pub fn inner(&self) -> Arc<DB> {
        Arc::clone(&self.db)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_database_creation() {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::new(temp_dir.path()).unwrap();
        
        // Verify column families exist
        assert!(db.db.cf_handle(CF_FILES).is_some());
        assert!(db.db.cf_handle(CF_MERKLE).is_some());
        assert!(db.db.cf_handle(CF_METADATA).is_some());
    }
    
    #[test]
    fn test_put_get_delete() {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::new(temp_dir.path()).unwrap();
        
        let key = b"test_key";
        let value = b"test_value";
        
        // Put value
        db.put(CF_FILES, key, value).unwrap();
        
        // Get value
        let retrieved = db.get(CF_FILES, key).unwrap();
        assert_eq!(retrieved, Some(value.to_vec()));
        
        // Check exists
        assert!(db.exists(CF_FILES, key).unwrap());
        
        // Delete value
        db.delete(CF_FILES, key).unwrap();
        
        // Verify deleted
        let retrieved = db.get(CF_FILES, key).unwrap();
        assert_eq!(retrieved, None);
        assert!(!db.exists(CF_FILES, key).unwrap());
    }
}
