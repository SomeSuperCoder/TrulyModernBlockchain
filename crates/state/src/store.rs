// FileStore implementation with RocksDB backend and LRU caching
use crate::MerkleTree;
use blockchain_common::{BlockchainError, File, FileID, Result, StateRoot};
use blockchain_storage::{Database, CF_FILES};
use lru::LruCache;
use sha2::{Digest, Sha256};
use std::num::NonZeroUsize;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// FileStore manages content-addressed files with persistent storage and caching
pub struct FileStore {
    /// RocksDB backend for persistent storage
    db: Arc<Database>,
    
    /// In-memory LRU cache for hot files (capacity: 10,000 files)
    cache: Arc<RwLock<LruCache<FileID, File>>>,
    
    /// Merkle tree for state root computation
    merkle_tree: Arc<RwLock<MerkleTree>>,
}

impl FileStore {
    /// Default cache capacity (10,000 files)
    const DEFAULT_CACHE_CAPACITY: usize = 10_000;
    
    /// Deletion fee in gas units
    const DELETION_FEE: u64 = 1_000;
    
    /// Create a new FileStore with the specified database path
    pub fn new(db_path: &str) -> Result<Self> {
        let db = Database::new(db_path)?;
        let cache_capacity = NonZeroUsize::new(Self::DEFAULT_CACHE_CAPACITY).unwrap();
        let cache = LruCache::new(cache_capacity);
        let merkle_tree = MerkleTree::new();
        
        Ok(Self {
            db: Arc::new(db),
            cache: Arc::new(RwLock::new(cache)),
            merkle_tree: Arc::new(RwLock::new(merkle_tree)),
        })
    }
    
    /// Create a new FileStore with an existing database instance
    pub fn with_database(db: Arc<Database>) -> Self {
        let cache_capacity = NonZeroUsize::new(Self::DEFAULT_CACHE_CAPACITY).unwrap();
        let cache = LruCache::new(cache_capacity);
        let merkle_tree = MerkleTree::new();
        
        Self {
            db,
            cache: Arc::new(RwLock::new(cache)),
            merkle_tree: Arc::new(RwLock::new(merkle_tree)),
        }
    }
    
    /// Create a new file with ID generation and validation
    /// Returns the generated FileID
    pub fn create_file(&self, mut file: File) -> Result<FileID> {
        // Generate File_ID if not set
        if file.id == FileID::default() {
            file.id = self.generate_file_id(&file)?;
        }
        
        // Validate file size
        file.validate_size()?;
        
        // Validate storage cost
        let required_balance = file.storage_cost();
        if file.balance < required_balance {
            return Err(BlockchainError::InvalidState(format!(
                "Insufficient balance: required {} but got {}",
                required_balance, file.balance
            )));
        }
        
        // Check if file already exists
        if self.db.exists(CF_FILES, file.id.as_bytes())? {
            return Err(BlockchainError::InvalidState(format!(
                "File with ID {:?} already exists",
                file.id
            )));
        }
        
        // Set timestamps
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| BlockchainError::InvalidState(format!("System time error: {}", e)))?
            .as_secs() as i64;
        file.created_at = now;
        file.updated_at = now;
        file.version = 0;
        
        // Generate nonce for collision prevention
        file.nonce = rand::random();
        
        // Serialize and store in database
        let key = file.id.as_bytes();
        let value = bincode::serialize(&file)
            .map_err(|e| BlockchainError::Serialization(format!("Failed to serialize file: {}", e)))?;
        self.db.put(CF_FILES, key, &value)?;
        
        // Update cache
        self.cache.write().unwrap().put(file.id, file.clone());
        
        // Update Merkle tree
        self.merkle_tree.write().unwrap().insert(file.id, file.compute_state_hash());
        
        Ok(file.id)
    }
    
    /// Get a file by ID with LRU caching
    /// Returns None if file does not exist
    pub fn get_file(&self, id: &FileID) -> Result<Option<File>> {
        // Check cache first
        {
            let mut cache = self.cache.write().unwrap();
            if let Some(file) = cache.get(id) {
                return Ok(Some(file.clone()));
            }
        }
        
        // Load from database
        let key = id.as_bytes();
        let value = match self.db.get(CF_FILES, key)? {
            Some(v) => v,
            None => return Ok(None),
        };
        
        let file: File = bincode::deserialize(&value)
            .map_err(|e| BlockchainError::Serialization(format!("Failed to deserialize file: {}", e)))?;
        
        // Update cache
        self.cache.write().unwrap().put(*id, file.clone());
        
        Ok(Some(file))
    }
    
    /// Update an existing file with version incrementing
    /// Validates storage cost and increments version number
    pub fn update_file(&self, mut file: File) -> Result<()> {
        // Validate file size
        file.validate_size()?;
        
        // Validate storage cost
        let required_balance = file.storage_cost();
        if file.balance < required_balance {
            return Err(BlockchainError::InvalidState(format!(
                "Insufficient balance: required {} but got {}",
                required_balance, file.balance
            )));
        }
        
        // Check if file exists
        if !self.db.exists(CF_FILES, file.id.as_bytes())? {
            return Err(BlockchainError::InvalidState(format!(
                "File with ID {:?} does not exist",
                file.id
            )));
        }
        
        // Increment version
        file.version += 1;
        
        // Update timestamp
        file.updated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| BlockchainError::InvalidState(format!("System time error: {}", e)))?
            .as_secs() as i64;
        
        // Serialize and store in database
        let key = file.id.as_bytes();
        let value = bincode::serialize(&file)
            .map_err(|e| BlockchainError::Serialization(format!("Failed to serialize file: {}", e)))?;
        self.db.put(CF_FILES, key, &value)?;
        
        // Update cache
        self.cache.write().unwrap().put(file.id, file.clone());
        
        // Update Merkle tree
        self.merkle_tree.write().unwrap().update(file.id, file.compute_state_hash());
        
        Ok(())
    }
    
    /// Delete a file with balance refund
    /// Returns the remaining balance minus deletion fee
    pub fn delete_file(&self, id: &FileID, refund_recipient: &FileID) -> Result<u64> {
        // Get the file
        let file = self.get_file(id)?
            .ok_or_else(|| BlockchainError::InvalidState(format!("File with ID {:?} does not exist", id)))?;
        
        // Calculate refund amount (balance minus deletion fee)
        let refund_amount = file.balance.saturating_sub(Self::DELETION_FEE);
        
        // Delete from database
        self.db.delete(CF_FILES, id.as_bytes())?;
        
        // Remove from cache
        self.cache.write().unwrap().pop(id);
        
        // Remove from Merkle tree
        self.merkle_tree.write().unwrap().remove(id);
        
        // If there's a refund and recipient is not the deleted file itself
        if refund_amount > 0 && refund_recipient != id {
            // Get recipient file
            if let Some(mut recipient_file) = self.get_file(refund_recipient)? {
                // Add refund to recipient balance
                recipient_file.balance = recipient_file.balance.saturating_add(refund_amount);
                self.update_file(recipient_file)?;
            }
        }
        
        Ok(refund_amount)
    }
    
    /// Generate a File_ID from file data and nonce
    /// Uses SHA-256(data || nonce)
    fn generate_file_id(&self, file: &File) -> Result<FileID> {
        let mut hasher = Sha256::new();
        hasher.update(&file.data);
        hasher.update(&file.nonce);
        Ok(FileID(hasher.finalize().into()))
    }
    
    /// Get the number of files in the store (for testing/debugging)
    pub fn file_count(&self) -> usize {
        self.cache.read().unwrap().len()
    }
    
    /// Compute the current state root from the Merkle tree
    pub fn compute_state_root(&self) -> StateRoot {
        self.merkle_tree.read().unwrap().root()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    fn create_test_file() -> File {
        File {
            id: FileID::default(),
            balance: 100_000,
            tx_manager: FileID::default(),
            data: vec![1, 2, 3, 4, 5],
            executable: false,
            version: 0,
            created_at: 0,
            updated_at: 0,
            nonce: [0; 16],
        }
    }
    
    #[test]
    fn test_create_file() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileStore::new(temp_dir.path().to_str().unwrap()).unwrap();
        
        let file = create_test_file();
        let file_id = store.create_file(file.clone()).unwrap();
        
        // Verify file was created
        assert_ne!(file_id, FileID::default());
        
        // Verify file can be retrieved
        let retrieved = store.get_file(&file_id).unwrap().unwrap();
        assert_eq!(retrieved.id, file_id);
        assert_eq!(retrieved.data, file.data);
        assert_eq!(retrieved.version, 0);
    }
    
    #[test]
    fn test_create_file_insufficient_balance() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileStore::new(temp_dir.path().to_str().unwrap()).unwrap();
        
        let mut file = create_test_file();
        file.balance = 100; // Too low
        
        let result = store.create_file(file);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_get_file_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileStore::new(temp_dir.path().to_str().unwrap()).unwrap();
        
        let file_id = FileID([1; 32]);
        let result = store.get_file(&file_id).unwrap();
        assert!(result.is_none());
    }
    
    #[test]
    fn test_update_file() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileStore::new(temp_dir.path().to_str().unwrap()).unwrap();
        
        // Create file
        let file = create_test_file();
        let file_id = store.create_file(file.clone()).unwrap();
        
        // Update file
        let mut updated_file = store.get_file(&file_id).unwrap().unwrap();
        updated_file.data = vec![10, 20, 30];
        store.update_file(updated_file.clone()).unwrap();
        
        // Verify update
        let retrieved = store.get_file(&file_id).unwrap().unwrap();
        assert_eq!(retrieved.data, vec![10, 20, 30]);
        assert_eq!(retrieved.version, 1); // Version incremented
    }
    
    #[test]
    fn test_delete_file() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileStore::new(temp_dir.path().to_str().unwrap()).unwrap();
        
        // Create file
        let file = create_test_file();
        let file_id = store.create_file(file.clone()).unwrap();
        
        // Create recipient for refund with different data
        let mut recipient_file = create_test_file();
        recipient_file.data = vec![99, 88, 77]; // Different data to get different ID
        let recipient_id = store.create_file(recipient_file.clone()).unwrap();
        let initial_recipient_balance = store.get_file(&recipient_id).unwrap().unwrap().balance;
        
        // Delete file
        let refund = store.delete_file(&file_id, &recipient_id).unwrap();
        
        // Verify file is deleted
        let result = store.get_file(&file_id).unwrap();
        assert!(result.is_none());
        
        // Verify refund
        assert_eq!(refund, file.balance - FileStore::DELETION_FEE);
        
        // Verify recipient received refund
        let recipient = store.get_file(&recipient_id).unwrap().unwrap();
        assert_eq!(recipient.balance, initial_recipient_balance + refund);
    }
    
    #[test]
    fn test_cache_functionality() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileStore::new(temp_dir.path().to_str().unwrap()).unwrap();
        
        // Create file
        let file = create_test_file();
        let file_id = store.create_file(file.clone()).unwrap();
        
        // First get (from database)
        let retrieved1 = store.get_file(&file_id).unwrap().unwrap();
        
        // Second get (from cache)
        let retrieved2 = store.get_file(&file_id).unwrap().unwrap();
        
        assert_eq!(retrieved1.id, retrieved2.id);
        assert_eq!(retrieved1.data, retrieved2.data);
    }
}
