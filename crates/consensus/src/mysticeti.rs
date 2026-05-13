use blockchain_common::types::{Block, BlockHash, ValidatorID, BlockHeader, AggregatedSignature, Transaction};
use blockchain_common::error::{BlockchainError, Result};
use std::collections::{HashMap, HashSet, VecDeque};

/// DAG node representing a block with its relationships
#[derive(Debug, Clone)]
pub struct DAGNode {
    /// The block itself
    pub block: Block,
    
    /// Hash of this block
    pub hash: BlockHash,
    
    /// Parent block hashes
    pub parents: Vec<BlockHash>,
    
    /// Child block hashes (blocks that reference this as parent)
    pub children: Vec<BlockHash>,
    
    /// Topological order (assigned during ordering)
    pub topo_order: Option<u64>,
    
    /// Whether this block has been finalized
    pub finalized: bool,
    
    /// Accumulated stake votes for this block
    pub stake_votes: u64,
}

impl DAGNode {
    pub fn new(block: Block) -> Self {
        let hash = block.hash();
        let parents = block.parents.clone();
        
        Self {
            block,
            hash,
            parents,
            children: Vec::new(),
            topo_order: None,
            finalized: false,
            stake_votes: 0,
        }
    }
}

/// Mysticeti DAG consensus implementation
/// Allows multiple validators to propose blocks simultaneously with deterministic ordering
pub struct MysticetDAG {
    /// All blocks in the DAG indexed by hash
    nodes: HashMap<BlockHash, DAGNode>,
    
    /// Genesis block hash
    genesis_hash: BlockHash,
    
    /// Latest finalized block hash
    latest_finalized: BlockHash,
    
    /// Blocks pending finalization (have enough votes but not yet marked final)
    pending_finalization: HashSet<BlockHash>,
    
    /// Total stake in the network (for calculating 2/3 threshold)
    total_stake: u64,
    
    /// Next topological order number to assign
    next_topo_order: u64,
}

impl MysticetDAG {
    /// Create a new DAG with a genesis block
    pub fn new(genesis_block: Block, total_stake: u64) -> Self {
        let genesis_hash = genesis_block.hash();
        let mut genesis_node = DAGNode::new(genesis_block);
        genesis_node.finalized = true;
        genesis_node.topo_order = Some(0);
        
        let mut nodes = HashMap::new();
        nodes.insert(genesis_hash, genesis_node);
        
        Self {
            nodes,
            genesis_hash,
            latest_finalized: genesis_hash,
            pending_finalization: HashSet::new(),
            total_stake,
            next_topo_order: 1,
        }
    }
    
    /// Add a block to the DAG with parent validation
    /// Returns error if parents don't exist or block is invalid
    pub fn add_block(&mut self, block: Block) -> Result<BlockHash> {
        let block_hash = block.hash();
        
        // Check if block already exists
        if self.nodes.contains_key(&block_hash) {
            return Err(BlockchainError::Consensus(
                "Block already exists in DAG".to_string()
            ));
        }
        
        // Validate all parents exist
        for parent_hash in &block.parents {
            if !self.nodes.contains_key(parent_hash) {
                return Err(BlockchainError::Consensus(
                    format!("Parent block {:?} not found in DAG", parent_hash)
                ));
            }
        }
        
        // Validate block has at least one parent (except genesis)
        if block.parents.is_empty() && block.header.height > 0 {
            return Err(BlockchainError::Consensus(
                "Non-genesis block must have at least one parent".to_string()
            ));
        }
        
        // Detect if this creates a fork
        self.detect_fork(&block)?;
        
        // Create DAG node
        let node = DAGNode::new(block.clone());
        
        // Update parent nodes to reference this as a child
        for parent_hash in &block.parents {
            if let Some(parent_node) = self.nodes.get_mut(parent_hash) {
                parent_node.children.push(block_hash);
            }
        }
        
        // Insert node into DAG
        self.nodes.insert(block_hash, node);
        
        // Assign topological order
        self.assign_topological_order(block_hash)?;
        
        Ok(block_hash)
    }
    
    /// Detect if adding this block would create a problematic fork
    /// Returns error if fork is invalid (e.g., conflicting blocks at same slot)
    fn detect_fork(&self, block: &Block) -> Result<()> {
        // Check for conflicting blocks at the same slot from the same proposer
        for (_, node) in &self.nodes {
            if node.block.header.slot == block.header.slot
                && node.block.header.proposer == block.header.proposer
                && node.hash != block.hash()
            {
                return Err(BlockchainError::Consensus(
                    format!(
                        "Fork detected: Validator {:?} produced conflicting blocks at slot {}",
                        block.header.proposer, block.header.slot
                    )
                ));
            }
        }
        
        Ok(())
    }
    
    /// Assign topological order to a block using DFS
    /// Ensures all parents are ordered before children
    fn assign_topological_order(&mut self, block_hash: BlockHash) -> Result<()> {
        // Get the block's parents
        let parents = if let Some(node) = self.nodes.get(&block_hash) {
            node.parents.clone()
        } else {
            return Err(BlockchainError::Consensus("Block not found".to_string()));
        };
        
        // Ensure all parents have topological orders
        for parent_hash in &parents {
            if let Some(parent_node) = self.nodes.get(parent_hash) {
                if parent_node.topo_order.is_none() {
                    return Err(BlockchainError::Consensus(
                        "Parent block has no topological order".to_string()
                    ));
                }
            }
        }
        
        // Assign order to this block
        if let Some(node) = self.nodes.get_mut(&block_hash) {
            node.topo_order = Some(self.next_topo_order);
            self.next_topo_order += 1;
        }
        
        Ok(())
    }
    
    /// Get blocks in topological order
    /// Returns blocks sorted such that parents always come before children
    pub fn get_topological_order(&self) -> Vec<BlockHash> {
        let mut ordered: Vec<(BlockHash, u64)> = self.nodes
            .iter()
            .filter_map(|(hash, node)| {
                node.topo_order.map(|order| (*hash, order))
            })
            .collect();
        
        // Sort by topological order
        ordered.sort_by_key(|(_, order)| *order);
        
        ordered.into_iter().map(|(hash, _)| hash).collect()
    }
    
    /// Get a block by hash
    pub fn get_block(&self, hash: &BlockHash) -> Option<&Block> {
        self.nodes.get(hash).map(|node| &node.block)
    }
    
    /// Get a DAG node by hash
    pub fn get_node(&self, hash: &BlockHash) -> Option<&DAGNode> {
        self.nodes.get(hash)
    }
    
    /// Get mutable DAG node by hash
    pub fn get_node_mut(&mut self, hash: &BlockHash) -> Option<&mut DAGNode> {
        self.nodes.get_mut(hash)
    }
    
    /// Check if a block exists in the DAG
    pub fn contains(&self, hash: &BlockHash) -> bool {
        self.nodes.contains_key(hash)
    }
    
    /// Get the genesis block hash
    pub fn genesis_hash(&self) -> BlockHash {
        self.genesis_hash
    }
    
    /// Get the latest finalized block hash
    pub fn latest_finalized_hash(&self) -> BlockHash {
        self.latest_finalized
    }
    
    /// Get all unfinalized blocks
    pub fn get_unfinalized_blocks(&self) -> Vec<BlockHash> {
        self.nodes
            .iter()
            .filter(|(_, node)| !node.finalized)
            .map(|(hash, _)| *hash)
            .collect()
    }
    
    /// Resolve fork by selecting the heaviest chain
    /// Returns the canonical chain tip based on accumulated stake votes
    pub fn resolve_fork(&self, competing_blocks: &[BlockHash]) -> Option<BlockHash> {
        if competing_blocks.is_empty() {
            return None;
        }
        
        // Select block with highest accumulated stake votes
        let mut best_block = competing_blocks[0];
        let mut best_stake = self.nodes.get(&best_block)?.stake_votes;
        
        for block_hash in &competing_blocks[1..] {
            if let Some(node) = self.nodes.get(block_hash) {
                if node.stake_votes > best_stake {
                    best_stake = node.stake_votes;
                    best_block = *block_hash;
                }
            }
        }
        
        Some(best_block)
    }
    
    /// Get all blocks at a specific slot
    pub fn get_blocks_at_slot(&self, slot: u64) -> Vec<BlockHash> {
        self.nodes
            .iter()
            .filter(|(_, node)| node.block.header.slot == slot)
            .map(|(hash, _)| *hash)
            .collect()
    }
    
    /// Add stake votes to a block
    pub fn add_stake_votes(&mut self, block_hash: &BlockHash, stake: u64) -> Result<()> {
        if let Some(node) = self.nodes.get_mut(block_hash) {
            node.stake_votes += stake;
            
            // Check if block now has 2/3 stake and should be finalized
            let threshold = (self.total_stake * 2) / 3;
            if node.stake_votes >= threshold && !node.finalized {
                self.pending_finalization.insert(*block_hash);
            }
            
            Ok(())
        } else {
            Err(BlockchainError::Consensus("Block not found".to_string()))
        }
    }
    
    /// Finalize a block (mark as irreversible)
    pub fn finalize_block(&mut self, block_hash: &BlockHash) -> Result<()> {
        if let Some(node) = self.nodes.get_mut(block_hash) {
            node.finalized = true;
            self.latest_finalized = *block_hash;
            self.pending_finalization.remove(block_hash);
            Ok(())
        } else {
            Err(BlockchainError::Consensus("Block not found".to_string()))
        }
    }
    
    /// Get blocks pending finalization
    pub fn get_pending_finalization(&self) -> Vec<BlockHash> {
        self.pending_finalization.iter().copied().collect()
    }
    
    /// Get the canonical chain from genesis to a specific block
    /// Returns blocks in order from genesis to target
    pub fn get_chain_to_block(&self, target: &BlockHash) -> Result<Vec<BlockHash>> {
        let mut chain = Vec::new();
        let mut current = *target;
        
        // Walk backwards from target to genesis
        while current != self.genesis_hash {
            chain.push(current);
            
            // Get the first parent (main chain)
            let node = self.nodes.get(&current)
                .ok_or_else(|| BlockchainError::Consensus("Block not found".to_string()))?;
            
            if node.parents.is_empty() {
                return Err(BlockchainError::Consensus(
                    "Block has no parents but is not genesis".to_string()
                ));
            }
            
            current = node.parents[0];
        }
        
        // Add genesis
        chain.push(self.genesis_hash);
        
        // Reverse to get genesis -> target order
        chain.reverse();
        
        Ok(chain)
    }
    
    /// Get all tips (blocks with no children)
    pub fn get_tips(&self) -> Vec<BlockHash> {
        self.nodes
            .iter()
            .filter(|(_, node)| node.children.is_empty())
            .map(|(hash, _)| *hash)
            .collect()
    }
    
    /// Prune finalized blocks older than a certain height
    /// Keeps only recent finalized blocks and all unfinalized blocks
    pub fn prune_old_blocks(&mut self, keep_recent_blocks: u64) -> Result<usize> {
        let latest_finalized_node = self.nodes.get(&self.latest_finalized)
            .ok_or_else(|| BlockchainError::Consensus("Latest finalized block not found".to_string()))?;
        
        let prune_before_height = latest_finalized_node.block.header.height.saturating_sub(keep_recent_blocks);
        
        // Collect blocks to prune
        let to_prune: Vec<BlockHash> = self.nodes
            .iter()
            .filter(|(_, node)| {
                node.finalized && node.block.header.height < prune_before_height
            })
            .map(|(hash, _)| *hash)
            .collect();
        
        let pruned_count = to_prune.len();
        
        // Remove pruned blocks
        for hash in to_prune {
            self.nodes.remove(&hash);
        }
        
        Ok(pruned_count)
    }
}

/// Block proposal and voting manager
/// Handles block creation, broadcasting, vote collection, and finalization
pub struct BlockProposer {
    /// Reference to the DAG
    dag: MysticetDAG,
    
    /// Validator information (ID and stake)
    validator_id: ValidatorID,
    validator_stake: u64,
    
    /// Collected votes for blocks (block_hash -> (validator_id, stake))
    votes: HashMap<BlockHash, HashMap<ValidatorID, u64>>,
    
    /// Blocks we've voted for (to prevent double voting)
    voted_blocks: HashSet<BlockHash>,
}

impl BlockProposer {
    /// Create a new block proposer
    pub fn new(dag: MysticetDAG, validator_id: ValidatorID, validator_stake: u64) -> Self {
        Self {
            dag,
            validator_id,
            validator_stake,
            votes: HashMap::new(),
            voted_blocks: HashSet::new(),
        }
    }
    
    /// Create a new block with ordered transactions
    /// Selects parent blocks from DAG tips and orders transactions deterministically
    pub fn create_block(
        &mut self,
        slot: u64,
        epoch: u64,
        transactions: Vec<Transaction>,
        state_root: blockchain_common::types::StateRoot,
    ) -> Result<Block> {
        // Get current DAG tips as parents
        let parents = self.dag.get_tips();
        
        if parents.is_empty() {
            return Err(BlockchainError::Consensus(
                "No parent blocks available".to_string()
            ));
        }
        
        // Determine block height (max parent height + 1)
        let height = parents.iter()
            .filter_map(|p| self.dag.get_block(p))
            .map(|b| b.header.height)
            .max()
            .unwrap_or(0) + 1;
        
        // Get previous block hash (first parent)
        let previous_hash = parents[0];
        
        // Order transactions deterministically by transaction ID
        let mut ordered_txs = transactions;
        ordered_txs.sort_by_key(|tx| tx.compute_id().0);
        
        // Create block
        let block = Block {
            header: BlockHeader {
                slot,
                height,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as i64,
                previous_hash,
                transactions_root: blockchain_common::types::MerkleRoot([0u8; 32]), // Will be computed
                state_root,
                proposer: self.validator_id,
                epoch,
                version: 1,
            },
            transactions: ordered_txs.clone(),
            signature: AggregatedSignature {
                signature: Vec::new(), // Will be filled after voting
                signers: Vec::new(),
            },
            parents,
        };
        
        // Compute transactions root
        let mut block_with_root = block;
        block_with_root.header.transactions_root = block_with_root.compute_transactions_root();
        
        Ok(block_with_root)
    }
    
    /// Broadcast a block to the network (placeholder for gossip integration)
    /// In production, this would send the block via the Elixir gossip layer
    pub fn broadcast_block(&self, _block: &Block) -> Result<()> {
        // TODO: Integrate with Elixir gossip protocol
        // For now, this is a placeholder that would serialize the block
        // and send it via Protocol Buffers to the networking layer
        
        // tracing::info!(
        //     "Broadcasting block at slot {} height {} with {} transactions",
        //     block.header.slot,
        //     block.header.height,
        //     block.transactions.len()
        // );
        
        Ok(())
    }
    
    /// Receive and validate a block from the network
    /// Adds block to DAG if valid
    pub fn receive_block(&mut self, block: Block) -> Result<BlockHash> {
        // Validate block structure
        self.validate_block(&block)?;
        
        // Add to DAG
        let block_hash = self.dag.add_block(block)?;
        
        // tracing::info!("Received and added block {:?} to DAG", block_hash);
        
        Ok(block_hash)
    }
    
    /// Validate a block before adding to DAG
    fn validate_block(&self, block: &Block) -> Result<()> {
        // Check block version
        if block.header.version == 0 {
            return Err(BlockchainError::Consensus(
                "Invalid block version".to_string()
            ));
        }
        
        // Verify transactions root
        let computed_root = block.compute_transactions_root();
        if computed_root != block.header.transactions_root {
            return Err(BlockchainError::Consensus(
                "Transactions root mismatch".to_string()
            ));
        }
        
        // Check timestamp is reasonable (not too far in future)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        
        if block.header.timestamp > now + 5000 {
            return Err(BlockchainError::Consensus(
                "Block timestamp too far in future".to_string()
            ));
        }
        
        Ok(())
    }
    
    /// Vote for a block (sign it with validator's key)
    /// Returns true if vote was recorded, false if already voted
    pub fn vote_for_block(&mut self, block_hash: &BlockHash) -> Result<bool> {
        // Check if we've already voted for this block
        if self.voted_blocks.contains(block_hash) {
            return Ok(false);
        }
        
        // Verify block exists in DAG
        if !self.dag.contains(block_hash) {
            return Err(BlockchainError::Consensus(
                "Cannot vote for block not in DAG".to_string()
            ));
        }
        
        // Record our vote
        self.voted_blocks.insert(*block_hash);
        
        // Add vote to collection
        self.votes
            .entry(*block_hash)
            .or_insert_with(HashMap::new)
            .insert(self.validator_id, self.validator_stake);
        
        // Update stake votes in DAG
        self.dag.add_stake_votes(block_hash, self.validator_stake)?;
        
        // tracing::info!(
        //     "Voted for block {:?} with stake {}",
        //     block_hash,
        //     self.validator_stake
        // );
        
        Ok(true)
    }
    
    /// Receive a vote from another validator
    pub fn receive_vote(
        &mut self,
        block_hash: &BlockHash,
        validator_id: ValidatorID,
        stake: u64,
    ) -> Result<()> {
        // Verify block exists
        if !self.dag.contains(block_hash) {
            return Err(BlockchainError::Consensus(
                "Cannot vote for block not in DAG".to_string()
            ));
        }
        
        // Check if this validator already voted for this block
        if let Some(votes) = self.votes.get(block_hash) {
            if votes.contains_key(&validator_id) {
                return Err(BlockchainError::Consensus(
                    "Validator already voted for this block".to_string()
                ));
            }
        }
        
        // Record vote
        self.votes
            .entry(*block_hash)
            .or_insert_with(HashMap::new)
            .insert(validator_id, stake);
        
        // Update stake votes in DAG
        self.dag.add_stake_votes(block_hash, stake)?;
        
        // tracing::debug!(
        //     "Received vote for block {:?} from validator {:?} with stake {}",
        //     block_hash,
        //     validator_id,
        //     stake
        // );
        
        Ok(())
    }
    
    /// Check if a block has reached 2/3 stake threshold for finalization
    pub fn check_finalization(&mut self, block_hash: &BlockHash) -> Result<bool> {
        let node = self.dag.get_node(block_hash)
            .ok_or_else(|| BlockchainError::Consensus("Block not found".to_string()))?;
        
        let threshold = (self.dag.total_stake * 2) / 3;
        let stake_votes = node.stake_votes;
        let finalized = node.finalized;
        
        if stake_votes >= threshold && !finalized {
            // Finalize the block
            self.dag.finalize_block(block_hash)?;
            
            // tracing::info!(
            //     "Block {:?} finalized with {} stake votes (threshold: {})",
            //     block_hash,
            //     stake_votes,
            //     threshold
            // );
            
            return Ok(true);
        }
        
        Ok(false)
    }
    
    /// Get total votes for a block
    pub fn get_vote_count(&self, block_hash: &BlockHash) -> u64 {
        self.votes
            .get(block_hash)
            .map(|votes| votes.values().sum())
            .unwrap_or(0)
    }
    
    /// Get all blocks that have reached finalization threshold but aren't finalized yet
    pub fn get_blocks_ready_for_finalization(&self) -> Vec<BlockHash> {
        self.dag.get_pending_finalization()
    }
    
    /// Process finalization for all eligible blocks
    pub fn process_finalization(&mut self) -> Result<Vec<BlockHash>> {
        let ready_blocks = self.get_blocks_ready_for_finalization();
        let mut finalized = Vec::new();
        
        for block_hash in ready_blocks {
            if self.check_finalization(&block_hash)? {
                finalized.push(block_hash);
            }
        }
        
        Ok(finalized)
    }
    
    /// Get reference to the DAG
    pub fn dag(&self) -> &MysticetDAG {
        &self.dag
    }
    
    /// Get mutable reference to the DAG
    pub fn dag_mut(&mut self) -> &mut MysticetDAG {
        &mut self.dag
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_genesis_block() -> Block {
        Block {
            header: BlockHeader {
                slot: 0,
                height: 0,
                timestamp: 0,
                previous_hash: BlockHash([0u8; 32]),
                transactions_root: blockchain_common::types::MerkleRoot([0u8; 32]),
                state_root: blockchain_common::types::StateRoot([0u8; 32]),
                proposer: ValidatorID([0u8; 32]),
                epoch: 0,
                version: 1,
            },
            transactions: Vec::new(),
            signature: AggregatedSignature {
                signature: Vec::new(),
                signers: Vec::new(),
            },
            parents: Vec::new(),
        }
    }
    
    #[test]
    fn test_dag_add_block() {
        let genesis = create_genesis_block();
        let mut dag = MysticetDAG::new(genesis.clone(), 1000);
        
        // Create a child block
        let mut child = genesis.clone();
        child.header.slot = 1;
        child.header.height = 1;
        child.parents = vec![genesis.hash()];
        
        let result = dag.add_block(child.clone());
        assert!(result.is_ok());
        
        // Verify block was added
        assert!(dag.contains(&child.hash()));
    }
    
    #[test]
    fn test_dag_topological_order() {
        let genesis = create_genesis_block();
        let mut dag = MysticetDAG::new(genesis.clone(), 1000);
        
        // Add blocks in sequence
        let mut prev_hash = genesis.hash();
        for i in 1..=5 {
            let mut block = genesis.clone();
            block.header.slot = i;
            block.header.height = i;
            block.parents = vec![prev_hash];
            
            prev_hash = block.hash();
            dag.add_block(block).unwrap();
        }
        
        // Get topological order
        let ordered = dag.get_topological_order();
        assert_eq!(ordered.len(), 6); // Genesis + 5 blocks
        
        // Verify order is correct (genesis first)
        assert_eq!(ordered[0], genesis.hash());
    }
    
    #[test]
    fn test_fork_detection() {
        let genesis = create_genesis_block();
        let mut dag = MysticetDAG::new(genesis.clone(), 1000);
        
        let proposer = ValidatorID([1u8; 32]);
        
        // Create first block at slot 1
        let mut block1 = genesis.clone();
        block1.header.slot = 1;
        block1.header.height = 1;
        block1.header.proposer = proposer;
        block1.parents = vec![genesis.hash()];
        
        dag.add_block(block1.clone()).unwrap();
        
        // Try to create conflicting block at same slot from same proposer
        let mut block2 = genesis.clone();
        block2.header.slot = 1;
        block2.header.height = 1;
        block2.header.proposer = proposer;
        block2.header.timestamp = 1000; // Different timestamp to create different hash
        block2.parents = vec![genesis.hash()];
        
        let result = dag.add_block(block2);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_voting_and_finalization() {
        let genesis = create_genesis_block();
        let total_stake = 1000u64;
        let dag = MysticetDAG::new(genesis.clone(), total_stake);
        
        let validator_id = ValidatorID([1u8; 32]);
        let validator_stake = 700u64; // 70% of total stake (> 2/3)
        
        let mut proposer = BlockProposer::new(dag, validator_id, validator_stake);
        
        // Create a block
        let mut block = genesis.clone();
        block.header.slot = 1;
        block.header.height = 1;
        block.parents = vec![genesis.hash()];
        
        let block_hash = proposer.receive_block(block).unwrap();
        
        // Vote for the block
        proposer.vote_for_block(&block_hash).unwrap();
        
        // Check finalization (should succeed with 70% stake)
        let finalized = proposer.check_finalization(&block_hash).unwrap();
        assert!(finalized);
    }
    
    #[test]
    fn test_block_proposal() {
        let genesis = create_genesis_block();
        let total_stake = 1000u64;
        let dag = MysticetDAG::new(genesis.clone(), total_stake);
        
        let validator_id = ValidatorID([1u8; 32]);
        let mut proposer = BlockProposer::new(dag, validator_id, 100);
        
        // Create a block
        let transactions: Vec<Transaction> = Vec::new();
        let state_root = blockchain_common::types::StateRoot([0u8; 32]);
        
        let block = proposer.create_block(1, 0, transactions, state_root).unwrap();
        
        assert_eq!(block.header.slot, 1);
        assert_eq!(block.header.height, 1);
        assert_eq!(block.header.proposer, validator_id);
    }
}

/// Fork choice rule implementation
/// Selects canonical chain based on accumulated stake votes
pub struct ForkChoice {
    /// Reference to the DAG
    dag: MysticetDAG,
    
    /// Finality checkpoints (height -> block_hash)
    finality_checkpoints: HashMap<u64, BlockHash>,
    
    /// Checkpoint interval (create checkpoint every N blocks)
    checkpoint_interval: u64,
}

impl ForkChoice {
    /// Create a new fork choice manager
    pub fn new(dag: MysticetDAG, checkpoint_interval: u64) -> Self {
        let mut checkpoints = HashMap::new();
        
        // Add genesis as first checkpoint
        let genesis_hash = dag.genesis_hash();
        if let Some(genesis_node) = dag.get_node(&genesis_hash) {
            checkpoints.insert(genesis_node.block.header.height, genesis_hash);
        }
        
        Self {
            dag,
            finality_checkpoints: checkpoints,
            checkpoint_interval,
        }
    }
    
    /// Select the canonical chain tip using heaviest chain rule
    /// Returns the block with the most accumulated stake votes
    pub fn select_canonical_tip(&self) -> Option<BlockHash> {
        // Get all tips (blocks with no children)
        let tips = self.dag.get_tips();
        
        if tips.is_empty() {
            return None;
        }
        
        // Select tip with highest accumulated stake
        let mut best_tip = tips[0];
        let mut best_stake = self.get_accumulated_stake(&best_tip);
        
        for tip in &tips[1..] {
            let stake = self.get_accumulated_stake(tip);
            if stake > best_stake {
                best_stake = stake;
                best_tip = *tip;
            }
        }
        
        Some(best_tip)
    }
    
    /// Get accumulated stake votes for a block and all its ancestors
    fn get_accumulated_stake(&self, block_hash: &BlockHash) -> u64 {
        let mut total_stake = 0u64;
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(*block_hash);
        
        while let Some(current) = queue.pop_front() {
            if visited.contains(&current) {
                continue;
            }
            visited.insert(current);
            
            if let Some(node) = self.dag.get_node(&current) {
                total_stake += node.stake_votes;
                
                // Add parents to queue
                for parent in &node.parents {
                    if !visited.contains(parent) {
                        queue.push_back(*parent);
                    }
                }
            }
        }
        
        total_stake
    }
    
    /// Resolve conflict between competing blocks at the same height
    /// Returns the block with more accumulated stake votes
    pub fn resolve_conflict(&self, block1: &BlockHash, block2: &BlockHash) -> Option<BlockHash> {
        let stake1 = self.get_accumulated_stake(block1);
        let stake2 = self.get_accumulated_stake(block2);
        
        if stake1 > stake2 {
            Some(*block1)
        } else if stake2 > stake1 {
            Some(*block2)
        } else {
            // Tie-breaker: use block hash (deterministic)
            if block1.0 < block2.0 {
                Some(*block1)
            } else {
                Some(*block2)
            }
        }
    }
    
    /// Get the canonical chain from genesis to the current tip
    pub fn get_canonical_chain(&self) -> Result<Vec<BlockHash>> {
        let tip = self.select_canonical_tip()
            .ok_or_else(|| BlockchainError::Consensus("No canonical tip found".to_string()))?;
        
        self.dag.get_chain_to_block(&tip)
    }
    
    /// Create a finality checkpoint at the current finalized block
    pub fn create_checkpoint(&mut self) -> Result<()> {
        let latest_finalized = self.dag.latest_finalized_hash();
        let node = self.dag.get_node(&latest_finalized)
            .ok_or_else(|| BlockchainError::Consensus("Latest finalized block not found".to_string()))?;
        
        let height = node.block.header.height;
        
        // Only create checkpoint if at interval boundary
        if height % self.checkpoint_interval == 0 {
            self.finality_checkpoints.insert(height, latest_finalized);
            
            // tracing::info!(
            //     "Created finality checkpoint at height {} for block {:?}",
            //     height,
            //     latest_finalized
            // );
        }
        
        Ok(())
    }
    
    /// Get the most recent finality checkpoint
    pub fn get_latest_checkpoint(&self) -> Option<(u64, BlockHash)> {
        self.finality_checkpoints
            .iter()
            .max_by_key(|(height, _)| *height)
            .map(|(h, b)| (*h, *b))
    }
    
    /// Get checkpoint at a specific height
    pub fn get_checkpoint_at_height(&self, height: u64) -> Option<BlockHash> {
        self.finality_checkpoints.get(&height).copied()
    }
    
    /// Get all checkpoints
    pub fn get_all_checkpoints(&self) -> Vec<(u64, BlockHash)> {
        let mut checkpoints: Vec<_> = self.finality_checkpoints
            .iter()
            .map(|(h, b)| (*h, *b))
            .collect();
        checkpoints.sort_by_key(|(h, _)| *h);
        checkpoints
    }
    
    /// Verify that a block is on the canonical chain
    pub fn is_on_canonical_chain(&self, block_hash: &BlockHash) -> Result<bool> {
        let canonical_chain = self.get_canonical_chain()?;
        Ok(canonical_chain.contains(block_hash))
    }
    
    /// Get competing blocks at a specific slot
    /// Returns blocks that could be competing for the same position
    pub fn get_competing_blocks(&self, slot: u64) -> Vec<BlockHash> {
        self.dag.get_blocks_at_slot(slot)
    }
    
    /// Resolve all conflicts at a specific slot
    /// Returns the winning block based on stake votes
    pub fn resolve_slot_conflicts(&self, slot: u64) -> Option<BlockHash> {
        let competing = self.get_competing_blocks(slot);
        
        if competing.is_empty() {
            return None;
        }
        
        if competing.len() == 1 {
            return Some(competing[0]);
        }
        
        // Find block with highest stake
        let mut best_block = competing[0];
        let mut best_stake = self.dag.get_node(&best_block)?.stake_votes;
        
        for block_hash in &competing[1..] {
            if let Some(node) = self.dag.get_node(block_hash) {
                if node.stake_votes > best_stake {
                    best_stake = node.stake_votes;
                    best_block = *block_hash;
                }
            }
        }
        
        Some(best_block)
    }
    
    /// Prune old checkpoints, keeping only recent ones
    pub fn prune_old_checkpoints(&mut self, keep_recent: u64) -> usize {
        let latest_height = self.get_latest_checkpoint()
            .map(|(h, _)| h)
            .unwrap_or(0);
        
        let prune_before = latest_height.saturating_sub(keep_recent * self.checkpoint_interval);
        
        let to_remove: Vec<u64> = self.finality_checkpoints
            .keys()
            .filter(|&&h| h < prune_before)
            .copied()
            .collect();
        
        let count = to_remove.len();
        for height in to_remove {
            self.finality_checkpoints.remove(&height);
        }
        
        count
    }
    
    /// Get reference to the DAG
    pub fn dag(&self) -> &MysticetDAG {
        &self.dag
    }
    
    /// Get mutable reference to the DAG
    pub fn dag_mut(&mut self) -> &mut MysticetDAG {
        &mut self.dag
    }
}

/// Consensus engine that combines DAG, block proposal, and fork choice
pub struct ConsensusEngine {
    /// Block proposer for creating and voting on blocks
    proposer: BlockProposer,
    
    /// Fork choice rule for selecting canonical chain
    fork_choice: ForkChoice,
}

impl ConsensusEngine {
    /// Create a new consensus engine
    pub fn new(
        genesis_block: Block,
        total_stake: u64,
        validator_id: ValidatorID,
        validator_stake: u64,
        checkpoint_interval: u64,
    ) -> Self {
        let dag = MysticetDAG::new(genesis_block, total_stake);
        let proposer = BlockProposer::new(dag, validator_id, validator_stake);
        
        // Create fork choice with a clone of the DAG
        // Note: In production, this would share the same DAG via Arc<RwLock<>>
        let genesis_block_for_fc = proposer.dag().get_block(&proposer.dag().genesis_hash())
            .unwrap()
            .clone();
        let dag_for_fc = MysticetDAG::new(genesis_block_for_fc, total_stake);
        let fork_choice = ForkChoice::new(dag_for_fc, checkpoint_interval);
        
        Self {
            proposer,
            fork_choice,
        }
    }
    
    /// Propose a new block
    pub fn propose_block(
        &mut self,
        slot: u64,
        epoch: u64,
        transactions: Vec<Transaction>,
        state_root: blockchain_common::types::StateRoot,
    ) -> Result<Block> {
        let block = self.proposer.create_block(slot, epoch, transactions, state_root)?;
        
        // Broadcast the block
        self.proposer.broadcast_block(&block)?;
        
        // Add to our own DAG
        let block_hash = self.proposer.receive_block(block.clone())?;
        
        // Vote for our own block
        self.proposer.vote_for_block(&block_hash)?;
        
        Ok(block)
    }
    
    /// Receive a block from the network
    pub fn receive_block(&mut self, block: Block) -> Result<BlockHash> {
        self.proposer.receive_block(block)
    }
    
    /// Receive a vote from another validator
    pub fn receive_vote(
        &mut self,
        block_hash: &BlockHash,
        validator_id: ValidatorID,
        stake: u64,
    ) -> Result<()> {
        self.proposer.receive_vote(block_hash, validator_id, stake)
    }
    
    /// Process finalization for all eligible blocks
    pub fn process_finalization(&mut self) -> Result<Vec<BlockHash>> {
        let finalized = self.proposer.process_finalization()?;
        
        // Create checkpoints for newly finalized blocks
        if !finalized.is_empty() {
            self.fork_choice.create_checkpoint()?;
        }
        
        Ok(finalized)
    }
    
    /// Get the canonical chain tip
    pub fn get_canonical_tip(&self) -> Option<BlockHash> {
        self.fork_choice.select_canonical_tip()
    }
    
    /// Get the canonical chain
    pub fn get_canonical_chain(&self) -> Result<Vec<BlockHash>> {
        self.fork_choice.get_canonical_chain()
    }
    
    /// Get the latest finalized block
    pub fn get_latest_finalized(&self) -> BlockHash {
        self.proposer.dag().latest_finalized_hash()
    }
    
    /// Get reference to the proposer
    pub fn proposer(&self) -> &BlockProposer {
        &self.proposer
    }
    
    /// Get reference to fork choice
    pub fn fork_choice(&self) -> &ForkChoice {
        &self.fork_choice
    }
}

#[cfg(test)]
mod fork_choice_tests {
    use super::*;
    
    fn create_test_block(slot: u64, height: u64, parents: Vec<BlockHash>) -> Block {
        Block {
            header: BlockHeader {
                slot,
                height,
                timestamp: slot as i64 * 400,
                previous_hash: if parents.is_empty() { BlockHash([0u8; 32]) } else { parents[0] },
                transactions_root: blockchain_common::types::MerkleRoot([0u8; 32]),
                state_root: blockchain_common::types::StateRoot([0u8; 32]),
                proposer: ValidatorID([0u8; 32]),
                epoch: 0,
                version: 1,
            },
            transactions: Vec::new(),
            signature: AggregatedSignature {
                signature: Vec::new(),
                signers: Vec::new(),
            },
            parents,
        }
    }
    
    #[test]
    fn test_fork_choice_canonical_tip() {
        let genesis = create_test_block(0, 0, Vec::new());
        let dag = MysticetDAG::new(genesis.clone(), 1000);
        let fork_choice = ForkChoice::new(dag, 100);
        
        // Genesis should be the canonical tip initially
        let tip = fork_choice.select_canonical_tip();
        assert_eq!(tip, Some(genesis.hash()));
    }
    
    #[test]
    fn test_finality_checkpoint_creation() {
        let genesis = create_test_block(0, 0, Vec::new());
        let mut dag = MysticetDAG::new(genesis.clone(), 1000);
        
        // Add and finalize a block at checkpoint interval
        let block = create_test_block(100, 100, vec![genesis.hash()]);
        let block_hash = dag.add_block(block.clone()).unwrap();
        dag.add_stake_votes(&block_hash, 700).unwrap();
        dag.finalize_block(&block_hash).unwrap();
        
        let mut fork_choice = ForkChoice::new(dag, 100);
        fork_choice.create_checkpoint().unwrap();
        
        // Verify checkpoint was created
        let checkpoint = fork_choice.get_checkpoint_at_height(100);
        assert_eq!(checkpoint, Some(block_hash));
    }
    
    #[test]
    fn test_conflict_resolution() {
        let genesis = create_test_block(0, 0, Vec::new());
        let mut dag = MysticetDAG::new(genesis.clone(), 1000);
        
        // Create two competing blocks at slot 1
        let mut block1 = create_test_block(1, 1, vec![genesis.hash()]);
        block1.header.proposer = ValidatorID([1u8; 32]);
        let hash1 = dag.add_block(block1).unwrap();
        dag.add_stake_votes(&hash1, 600).unwrap();
        
        let mut block2 = create_test_block(1, 1, vec![genesis.hash()]);
        block2.header.proposer = ValidatorID([2u8; 32]);
        block2.header.timestamp += 1; // Make it different
        let hash2 = dag.add_block(block2).unwrap();
        dag.add_stake_votes(&hash2, 400).unwrap();
        
        let fork_choice = ForkChoice::new(dag, 100);
        
        // Block1 should win (more stake)
        let winner = fork_choice.resolve_conflict(&hash1, &hash2);
        assert_eq!(winner, Some(hash1));
    }
    
    #[test]
    fn test_consensus_engine_integration() {
        let genesis = create_test_block(0, 0, Vec::new());
        let validator_id = ValidatorID([1u8; 32]);
        
        let mut engine = ConsensusEngine::new(
            genesis.clone(),
            1000,
            validator_id,
            700,
            100,
        );
        
        // Propose a block
        let transactions = Vec::new();
        let state_root = blockchain_common::types::StateRoot([0u8; 32]);
        
        let _block = engine.propose_block(1, 0, transactions, state_root).unwrap();
        
        // Process finalization (should finalize with 70% stake)
        let finalized = engine.process_finalization().unwrap();
        assert!(!finalized.is_empty());
        
        // Verify canonical chain
        let chain = engine.get_canonical_chain().unwrap();
        assert!(chain.len() >= 2); // Genesis + new block
    }
}
