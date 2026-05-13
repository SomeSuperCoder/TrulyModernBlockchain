use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};

/// 32-byte file identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
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

/// Content-addressed state object with permanent ID and version-specific content hash
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct File {
    /// Permanent 32-byte identifier
    pub id: FileID,
    
    /// Balance in smallest unit (electrons)
    /// 1 Neon = 1,000,000 electrons
    pub balance: u64,
    
    /// Program that manages this file
    pub tx_manager: FileID,
    
    /// Arbitrary data (max 10MB)
    pub data: Vec<u8>,
    
    /// Whether this file contains executable code
    pub executable: bool,
    
    /// Monotonically increasing version number
    pub version: u64,
    
    /// Timestamp of creation (Unix seconds)
    pub created_at: i64,
    
    /// Timestamp of last update (Unix seconds)
    pub updated_at: i64,
    
    /// Random nonce for collision prevention
    pub nonce: [u8; 16],
}

impl File {
    /// Maximum file size in bytes (10MB)
    pub const MAX_SIZE: usize = 10 * 1024 * 1024;
    
    /// Minimum file creation cost in gas units
    pub const MIN_CREATION_COST: u64 = 10_000;
    
    /// Per-file creation fee in gas units
    pub const PER_FILE_FEE: u64 = 5_000;
    
    /// Base cost per KB in gas units
    pub const BASE_COST_PER_KB: u64 = 1_000;
    
    /// Compute the state hash for this file using SHA-256
    /// Hash includes: ID, balance, tx_manager, data, executable, version, updated_at, nonce
    pub fn compute_state_hash(&self) -> StateHash {
        let mut hasher = Sha256::new();
        hasher.update(&self.id.0);
        hasher.update(&self.balance.to_le_bytes());
        hasher.update(&self.tx_manager.0);
        hasher.update(&self.data);
        hasher.update(&[self.executable as u8]);
        hasher.update(&self.version.to_le_bytes());
        hasher.update(&self.updated_at.to_le_bytes());
        hasher.update(&self.nonce);
        StateHash(hasher.finalize().into())
    }
    
    /// Calculate storage cost for this file with exponential growth
    /// Formula: base_cost_per_kb × size_in_kb × (1.1 ^ size_in_mb) + min_cost + per_file_fee
    pub fn storage_cost(&self) -> u64 {
        let size_kb = (self.data.len() as f64 / 1024.0).ceil();
        let size_mb = (self.data.len() as f64 / (1024.0 * 1024.0)).ceil();
        
        let exponential_factor = 1.1_f64.powf(size_mb);
        let cost = (Self::BASE_COST_PER_KB as f64 * size_kb * exponential_factor) as u64;
        
        // Apply minimum cost and per-file fee
        cost.max(Self::MIN_CREATION_COST) + Self::PER_FILE_FEE
    }
    
    /// Validate that the file size does not exceed maximum
    pub fn validate_size(&self) -> Result<(), crate::error::BlockchainError> {
        if self.data.len() > Self::MAX_SIZE {
            return Err(crate::error::BlockchainError::InvalidState(
                format!("File size {} exceeds maximum of {} bytes", self.data.len(), Self::MAX_SIZE)
            ));
        }
        Ok(())
    }
}

impl Default for File {
    fn default() -> Self {
        Self {
            id: FileID::default(),
            balance: 0,
            tx_manager: FileID::default(),
            data: Vec::new(),
            executable: false,
            version: 0,
            created_at: 0,
            updated_at: 0,
            nonce: [0; 16],
        }
    }
}

/// Uniquely identifies a specific version of a File
/// Used for content-addressed state management and automatic conflict detection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObjectReference {
    /// Permanent file identifier
    pub file_id: FileID,
    
    /// Version number
    pub version: u64,
    
    /// Content hash of this specific version
    pub state_hash: StateHash,
}

impl ObjectReference {
    /// Create a new ObjectReference from a File
    pub fn from_file(file: &File) -> Self {
        Self {
            file_id: file.id,
            version: file.version,
            state_hash: file.compute_state_hash(),
        }
    }
    
    /// Verify this reference matches the current file state
    /// Returns true if file_id, version, and state_hash all match
    pub fn verify(&self, file: &File) -> bool {
        self.file_id == file.id
            && self.version == file.version
            && self.state_hash == file.compute_state_hash()
    }
}

/// Access permission for file operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessPermission {
    Read = 1,
    Write = 2,
}

/// File access specification for an instruction
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileAccess {
    pub file_id: FileID,
    pub permission: AccessPermission,
}

/// Instruction to execute within a transaction
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Instruction {
    /// Program to execute
    pub program_id: FileID,
    
    /// Files this instruction will access
    pub file_accesses: Vec<FileAccess>,
    
    /// Instruction-specific data
    pub data: Vec<u8>,
}

impl Instruction {
    /// Serialize instruction for hashing
    pub fn serialize(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

/// Cryptographic signature
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature {
    /// Public key that signed
    pub pubkey: [u8; 32],
    
    /// Signature bytes
    pub signature: Vec<u8>,
}

/// Zero-knowledge proof for client-side execution
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZKProof {
    /// Proof data
    pub proof: Vec<u8>,
    
    /// Input state hash
    pub input_state_hash: StateHash,
    
    /// Output state hash
    pub output_state_hash: StateHash,
}

/// Transaction containing instructions to execute
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transaction {
    /// Object references this transaction consumes
    pub inputs: Vec<ObjectReference>,
    
    /// Instructions to execute
    pub instructions: Vec<Instruction>,
    
    /// Signatures authorizing this transaction
    pub signatures: Vec<Signature>,
    
    /// Maximum gas this transaction can consume
    pub gas_limit: u64,
    
    /// Gas price in electrons per unit
    pub gas_price: u64,
    
    /// Optional ZK proof for client-side execution
    pub zk_proof: Option<ZKProof>,
}

impl Transaction {
    /// Fixed gas cost for ZK proof verification
    pub const ZK_PROOF_VERIFICATION_COST: u64 = 100_000;
    
    /// Compute transaction ID by hashing inputs and instructions
    pub fn compute_id(&self) -> TransactionID {
        let mut hasher = Sha256::new();
        
        // Hash all inputs
        for input in &self.inputs {
            hasher.update(&input.file_id.0);
            hasher.update(&input.version.to_le_bytes());
            hasher.update(&input.state_hash.0);
        }
        
        // Hash all instructions
        for instruction in &self.instructions {
            hasher.update(&instruction.serialize());
        }
        
        TransactionID(hasher.finalize().into())
    }
    
    /// Calculate total fee for this transaction
    /// Returns fixed cost for ZK proofs, or gas_limit * gas_price for traditional execution
    pub fn calculate_fee(&self) -> u64 {
        if self.zk_proof.is_some() {
            // Fixed cost for ZK proof verification
            Self::ZK_PROOF_VERIFICATION_COST
        } else {
            // Will be determined after execution, but this is the maximum
            self.gas_limit * self.gas_price
        }
    }
    
    /// Check if transaction has write access to a specific file
    pub fn has_write_access(&self, file_id: &FileID) -> bool {
        self.instructions.iter().any(|instr| {
            instr.file_accesses.iter().any(|access| {
                access.file_id == *file_id && access.permission == AccessPermission::Write
            })
        })
    }
}

/// 32-byte Merkle root hash
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MerkleRoot(pub [u8; 32]);

/// 32-byte state root hash
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StateRoot(pub [u8; 32]);

impl Default for StateRoot {
    fn default() -> Self {
        Self([0u8; 32])
    }
}

/// Aggregated BLS signature from validators
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AggregatedSignature {
    /// Aggregated signature bytes (96 bytes for BLS12-381)
    pub signature: Vec<u8>,
    
    /// Validator IDs that contributed to this signature
    pub signers: Vec<ValidatorID>,
}

/// Block header containing metadata
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockHeader {
    /// Slot number (400ms intervals)
    pub slot: u64,
    
    /// Block height (sequential)
    pub height: u64,
    
    /// Timestamp (Unix milliseconds)
    pub timestamp: i64,
    
    /// Hash of previous block
    pub previous_hash: BlockHash,
    
    /// Merkle root of transactions
    pub transactions_root: MerkleRoot,
    
    /// State root after executing this block
    pub state_root: StateRoot,
    
    /// Validator who produced this block
    pub proposer: ValidatorID,
    
    /// Epoch number
    pub epoch: u64,
    
    /// Block version for protocol upgrades
    pub version: u32,
}

impl BlockHeader {
    /// Compute the hash of this block header
    pub fn hash(&self) -> BlockHash {
        let mut hasher = Sha256::new();
        hasher.update(&self.slot.to_le_bytes());
        hasher.update(&self.height.to_le_bytes());
        hasher.update(&self.timestamp.to_le_bytes());
        hasher.update(&self.previous_hash.0);
        hasher.update(&self.transactions_root.0);
        hasher.update(&self.state_root.0);
        hasher.update(&self.proposer.0);
        hasher.update(&self.epoch.to_le_bytes());
        hasher.update(&self.version.to_le_bytes());
        BlockHash(hasher.finalize().into())
    }
}

/// Block containing header, transactions, and consensus data
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    /// Block header with metadata
    pub header: BlockHeader,
    
    /// Transactions in this block
    pub transactions: Vec<Transaction>,
    
    /// Aggregated BLS signature from validators
    pub signature: AggregatedSignature,
    
    /// References to parent blocks in DAG (for Mysticeti consensus)
    pub parents: Vec<BlockHash>,
}

impl Block {
    /// Compute the hash of this block (hash of header)
    pub fn hash(&self) -> BlockHash {
        self.header.hash()
    }
    
    /// Compute the Merkle root of all transactions
    pub fn compute_transactions_root(&self) -> MerkleRoot {
        if self.transactions.is_empty() {
            return MerkleRoot([0u8; 32]);
        }
        
        // Collect transaction IDs
        let mut hashes: Vec<[u8; 32]> = self.transactions
            .iter()
            .map(|tx| tx.compute_id().0)
            .collect();
        
        // Build Merkle tree bottom-up
        while hashes.len() > 1 {
            let mut next_level = Vec::new();
            
            for chunk in hashes.chunks(2) {
                let hash = if chunk.len() == 2 {
                    // Hash pair
                    let mut hasher = Sha256::new();
                    hasher.update(&chunk[0]);
                    hasher.update(&chunk[1]);
                    hasher.finalize().into()
                } else {
                    // Odd number, hash with itself
                    let mut hasher = Sha256::new();
                    hasher.update(&chunk[0]);
                    hasher.update(&chunk[0]);
                    hasher.finalize().into()
                };
                next_level.push(hash);
            }
            
            hashes = next_level;
        }
        
        MerkleRoot(hashes[0])
    }
}
