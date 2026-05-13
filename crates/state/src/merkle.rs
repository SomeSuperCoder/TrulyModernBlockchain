// Merkle tree implementation for state root computation
use blockchain_common::{FileID, StateHash, StateRoot};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Merkle tree for computing state root from file state hashes
/// Uses sorted leaf storage for deterministic ordering
pub struct MerkleTree {
    /// Leaf nodes (File_ID -> State_Hash) stored in sorted order
    leaves: BTreeMap<FileID, StateHash>,
}

impl MerkleTree {
    /// Create a new empty Merkle tree
    pub fn new() -> Self {
        Self {
            leaves: BTreeMap::new(),
        }
    }
    
    /// Insert a new leaf into the tree
    /// Invalidates cached internal nodes
    pub fn insert(&mut self, file_id: FileID, state_hash: StateHash) {
        self.leaves.insert(file_id, state_hash);
    }
    
    /// Update an existing leaf in the tree
    /// Invalidates cached internal nodes
    pub fn update(&mut self, file_id: FileID, state_hash: StateHash) {
        self.leaves.insert(file_id, state_hash);
    }
    
    /// Remove a leaf from the tree
    pub fn remove(&mut self, file_id: &FileID) {
        self.leaves.remove(file_id);
    }
    
    /// Compute the Merkle root with deterministic ordering
    /// Returns default StateRoot if tree is empty
    pub fn root(&self) -> StateRoot {
        if self.leaves.is_empty() {
            return StateRoot::default();
        }
        
        // Collect all leaf hashes in sorted order (BTreeMap maintains order)
        let mut hashes: Vec<[u8; 32]> = self.leaves
            .values()
            .map(|h| h.0)
            .collect();
        
        // Build tree bottom-up
        while hashes.len() > 1 {
            let mut next_level = Vec::new();
            
            for chunk in hashes.chunks(2) {
                let hash = if chunk.len() == 2 {
                    // Hash pair of nodes
                    let mut hasher = Sha256::new();
                    hasher.update(&chunk[0]);
                    hasher.update(&chunk[1]);
                    hasher.finalize().into()
                } else {
                    // Odd number of nodes, hash single node with itself
                    let mut hasher = Sha256::new();
                    hasher.update(&chunk[0]);
                    hasher.update(&chunk[0]);
                    hasher.finalize().into()
                };
                next_level.push(hash);
            }
            
            hashes = next_level;
        }
        
        StateRoot(hashes[0])
    }
    
    /// Generate a Merkle proof for a specific file
    /// Returns None if file is not in the tree
    pub fn generate_proof(&self, file_id: &FileID) -> Option<MerkleProof> {
        // Check if file exists in tree
        let target_hash = self.leaves.get(file_id)?;
        
        // Collect all leaf hashes in sorted order
        let leaf_hashes: Vec<([u8; 32], FileID)> = self.leaves
            .iter()
            .map(|(id, hash)| (hash.0, *id))
            .collect();
        
        // Find the index of our target
        let target_index = leaf_hashes
            .iter()
            .position(|(_, id)| id == file_id)?;
        
        // Build proof by collecting sibling hashes at each level
        let mut proof_hashes = Vec::new();
        let mut current_hashes: Vec<[u8; 32]> = leaf_hashes.iter().map(|(h, _)| *h).collect();
        let mut current_index = target_index;
        
        while current_hashes.len() > 1 {
            // Get sibling index
            let sibling_index = if current_index % 2 == 0 {
                current_index + 1
            } else {
                current_index - 1
            };
            
            // Add sibling hash to proof if it exists
            if sibling_index < current_hashes.len() {
                proof_hashes.push(current_hashes[sibling_index]);
            } else {
                // No sibling (odd number of nodes), use same hash
                proof_hashes.push(current_hashes[current_index]);
            }
            
            // Move to next level
            let mut next_level = Vec::new();
            for chunk in current_hashes.chunks(2) {
                let hash = if chunk.len() == 2 {
                    let mut hasher = Sha256::new();
                    hasher.update(&chunk[0]);
                    hasher.update(&chunk[1]);
                    hasher.finalize().into()
                } else {
                    let mut hasher = Sha256::new();
                    hasher.update(&chunk[0]);
                    hasher.update(&chunk[0]);
                    hasher.finalize().into()
                };
                next_level.push(hash);
            }
            
            current_hashes = next_level;
            current_index /= 2;
        }
        
        Some(MerkleProof {
            file_id: *file_id,
            state_hash: *target_hash,
            proof_hashes,
            leaf_index: target_index,
        })
    }
    
    /// Get the number of leaves in the tree
    pub fn len(&self) -> usize {
        self.leaves.len()
    }
    
    /// Check if the tree is empty
    pub fn is_empty(&self) -> bool {
        self.leaves.is_empty()
    }
}

impl Default for MerkleTree {
    fn default() -> Self {
        Self::new()
    }
}

/// Merkle proof for verifying file inclusion in state
#[derive(Debug, Clone)]
pub struct MerkleProof {
    /// File ID being proven
    pub file_id: FileID,
    
    /// State hash of the file
    pub state_hash: StateHash,
    
    /// Sibling hashes along the path to root
    pub proof_hashes: Vec<[u8; 32]>,
    
    /// Index of the leaf in the sorted tree
    pub leaf_index: usize,
}

impl MerkleProof {
    /// Verify this proof against a state root
    /// Returns true if the proof is valid
    pub fn verify(&self, state_root: &StateRoot) -> bool {
        let mut current_hash = self.state_hash.0;
        let mut current_index = self.leaf_index;
        
        // Reconstruct root by hashing with siblings
        for sibling_hash in &self.proof_hashes {
            let mut hasher = Sha256::new();
            
            // Hash in correct order based on index
            if current_index % 2 == 0 {
                // Current is left child
                hasher.update(&current_hash);
                hasher.update(sibling_hash);
            } else {
                // Current is right child
                hasher.update(sibling_hash);
                hasher.update(&current_hash);
            }
            
            current_hash = hasher.finalize().into();
            current_index /= 2;
        }
        
        // Check if computed root matches expected root
        current_hash == state_root.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_test_file_id(byte: u8) -> FileID {
        FileID([byte; 32])
    }
    
    fn create_test_state_hash(byte: u8) -> StateHash {
        StateHash([byte; 32])
    }
    
    #[test]
    fn test_empty_tree() {
        let tree = MerkleTree::new();
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
        assert_eq!(tree.root(), StateRoot::default());
    }
    
    #[test]
    fn test_single_leaf() {
        let mut tree = MerkleTree::new();
        let file_id = create_test_file_id(1);
        let state_hash = create_test_state_hash(1);
        
        tree.insert(file_id, state_hash);
        
        assert_eq!(tree.len(), 1);
        assert!(!tree.is_empty());
        
        // Root should be hash of single leaf with itself
        let root = tree.root();
        assert_ne!(root, StateRoot::default());
    }
    
    #[test]
    fn test_multiple_leaves() {
        let mut tree = MerkleTree::new();
        
        for i in 1..=5 {
            tree.insert(create_test_file_id(i), create_test_state_hash(i));
        }
        
        assert_eq!(tree.len(), 5);
        let root = tree.root();
        assert_ne!(root, StateRoot::default());
    }
    
    #[test]
    fn test_deterministic_ordering() {
        let mut tree1 = MerkleTree::new();
        let mut tree2 = MerkleTree::new();
        
        // Insert in different orders
        tree1.insert(create_test_file_id(1), create_test_state_hash(1));
        tree1.insert(create_test_file_id(2), create_test_state_hash(2));
        tree1.insert(create_test_file_id(3), create_test_state_hash(3));
        
        tree2.insert(create_test_file_id(3), create_test_state_hash(3));
        tree2.insert(create_test_file_id(1), create_test_state_hash(1));
        tree2.insert(create_test_file_id(2), create_test_state_hash(2));
        
        // Roots should be identical due to sorted ordering
        assert_eq!(tree1.root(), tree2.root());
    }
    
    #[test]
    fn test_update_leaf() {
        let mut tree = MerkleTree::new();
        let file_id = create_test_file_id(1);
        
        tree.insert(file_id, create_test_state_hash(1));
        let root1 = tree.root();
        
        tree.update(file_id, create_test_state_hash(2));
        let root2 = tree.root();
        
        // Root should change after update
        assert_ne!(root1, root2);
    }
    
    #[test]
    fn test_remove_leaf() {
        let mut tree = MerkleTree::new();
        let file_id = create_test_file_id(1);
        
        tree.insert(file_id, create_test_state_hash(1));
        assert_eq!(tree.len(), 1);
        
        tree.remove(&file_id);
        assert_eq!(tree.len(), 0);
        assert!(tree.is_empty());
    }
    
    #[test]
    fn test_generate_and_verify_proof() {
        let mut tree = MerkleTree::new();
        
        // Insert multiple leaves
        for i in 1..=4 {
            tree.insert(create_test_file_id(i), create_test_state_hash(i));
        }
        
        let root = tree.root();
        let file_id = create_test_file_id(2);
        
        // Generate proof
        let proof = tree.generate_proof(&file_id).unwrap();
        
        // Verify proof
        assert!(proof.verify(&root));
    }
    
    #[test]
    fn test_proof_for_nonexistent_file() {
        let mut tree = MerkleTree::new();
        tree.insert(create_test_file_id(1), create_test_state_hash(1));
        
        let file_id = create_test_file_id(99);
        let proof = tree.generate_proof(&file_id);
        
        assert!(proof.is_none());
    }
    
    #[test]
    fn test_invalid_proof() {
        let mut tree = MerkleTree::new();
        
        for i in 1..=4 {
            tree.insert(create_test_file_id(i), create_test_state_hash(i));
        }
        
        let root = tree.root();
        let file_id = create_test_file_id(2);
        let mut proof = tree.generate_proof(&file_id).unwrap();
        
        // Tamper with proof
        proof.state_hash = create_test_state_hash(99);
        
        // Verification should fail
        assert!(!proof.verify(&root));
    }
}
