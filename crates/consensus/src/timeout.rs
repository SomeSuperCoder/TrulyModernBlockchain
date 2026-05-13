use blockchain_common::types::{Block, ValidatorID};
use blockchain_common::error::{BlockchainError, Result};
use crate::bls::{BlsSignature, SignatureAggregator, ValidatorInfo as BlsValidatorInfo, AggregatedBlsSignature};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use std::collections::HashSet;

/// Result of waiting for a block
#[derive(Debug, Clone)]
pub enum TimeoutResult {
    /// Block was received before timeout
    BlockReceived(Block),
    
    /// Timeout expired, need to escalate to backup leader
    Timeout,
}

/// Timeout manager for handling leader failures and escalation
/// 
/// Implements Requirements 5.2, 5.3, 5.7:
/// - Configurable base timeout (minimum 800ms)
/// - Deadline tracking for block reception
/// - 50% timeout increase per backup leader
/// - Minimum timeout = 2× 99th percentile latency (floor 800ms)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutManager {
    /// Base timeout duration (2× 99th percentile latency, floor 800ms)
    base_timeout: Duration,
    
    /// Current timeout for this slot (increases with each backup)
    current_timeout: Duration,
    
    /// Current backup leader index (0 = primary, 1+ = backups)
    backup_index: u32,
    
    /// 99th percentile network latency in milliseconds
    p99_latency_ms: u64,
}

impl TimeoutManager {
    /// Minimum base timeout in milliseconds (800ms = 2 slots)
    pub const MIN_BASE_TIMEOUT_MS: u64 = 800;
    
    /// Timeout escalation factor (50% increase per backup)
    pub const ESCALATION_FACTOR: f64 = 1.5;
    
    /// Create a new timeout manager
    /// 
    /// # Arguments
    /// * `p99_latency_ms` - 99th percentile network latency in milliseconds
    /// 
    /// # Implementation
    /// Base timeout = max(2 × p99_latency_ms, 800ms)
    pub fn new(p99_latency_ms: u64) -> Self {
        let base_timeout_ms = (2 * p99_latency_ms).max(Self::MIN_BASE_TIMEOUT_MS);
        let base_timeout = Duration::from_millis(base_timeout_ms);
        
        Self {
            base_timeout,
            current_timeout: base_timeout,
            backup_index: 0,
            p99_latency_ms,
        }
    }
    
    /// Create with default timeout (800ms)
    pub fn with_default_timeout() -> Self {
        Self::new(400) // 400ms p99 latency → 800ms base timeout
    }
    
    /// Wait for a block with deadline tracking
    /// 
    /// This is a synchronous interface that would be called by the consensus engine.
    /// In a real implementation, this would integrate with the networking layer
    /// to receive blocks via channels or async streams.
    /// 
    /// # Arguments
    /// * `slot` - The slot number we're waiting for
    /// * `receive_fn` - Function that attempts to receive a block (non-blocking)
    /// 
    /// # Returns
    /// TimeoutResult indicating whether block was received or timeout occurred
    pub fn wait_for_block<F>(&mut self, slot: u64, mut receive_fn: F) -> TimeoutResult
    where
        F: FnMut() -> Option<Block>,
    {
        let deadline = Instant::now() + self.current_timeout;
        
        // Poll for block until deadline
        loop {
            // Try to receive block
            if let Some(block) = receive_fn() {
                // Verify block is for the correct slot
                if block.header.slot == slot {
                    return TimeoutResult::BlockReceived(block);
                }
            }
            
            // Check if deadline has passed
            if Instant::now() >= deadline {
                // Timeout expired, escalate to backup
                self.escalate();
                return TimeoutResult::Timeout;
            }
            
            // Small sleep to avoid busy-waiting
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    
    /// Escalate to the next backup leader
    /// 
    /// Increases backup_index and applies 50% timeout increase
    fn escalate(&mut self) {
        self.backup_index += 1;
        
        // Increase timeout by 50% for next backup
        let new_timeout_ms = (self.current_timeout.as_millis() as f64 * Self::ESCALATION_FACTOR) as u64;
        self.current_timeout = Duration::from_millis(new_timeout_ms);
    }
    
    /// Reset timeout manager for a new slot
    /// 
    /// Resets backup_index to 0 and current_timeout to base_timeout
    pub fn reset(&mut self) {
        self.backup_index = 0;
        self.current_timeout = self.base_timeout;
    }
    
    /// Get the current backup index
    /// 
    /// Returns 0 for primary leader, 1+ for backup leaders
    pub fn backup_index(&self) -> u32 {
        self.backup_index
    }
    
    /// Get the current timeout duration
    pub fn current_timeout(&self) -> Duration {
        self.current_timeout
    }
    
    /// Get the base timeout duration
    pub fn base_timeout(&self) -> Duration {
        self.base_timeout
    }
    
    /// Update the base timeout based on new network latency measurements
    /// 
    /// # Arguments
    /// * `p99_latency_ms` - New 99th percentile network latency in milliseconds
    pub fn update_base_timeout(&mut self, p99_latency_ms: u64) {
        self.p99_latency_ms = p99_latency_ms;
        let base_timeout_ms = (2 * p99_latency_ms).max(Self::MIN_BASE_TIMEOUT_MS);
        self.base_timeout = Duration::from_millis(base_timeout_ms);
        
        // Reset current timeout if we're at primary leader
        if self.backup_index == 0 {
            self.current_timeout = self.base_timeout;
        }
    }
    
    /// Check if timeout has expired for a given deadline
    pub fn is_expired(&self, deadline: Instant) -> bool {
        Instant::now() >= deadline
    }
    
    /// Calculate deadline for current timeout
    pub fn calculate_deadline(&self) -> Instant {
        Instant::now() + self.current_timeout
    }
}

/// Timeout certificate proving that validators timed out waiting for primary leader
/// 
/// Implements Requirements 5.4, 5.5, 5.8:
/// - Requires 2/3 stake signatures
/// - Validates that timeout was not premature
/// - Rejects conflicting votes (validators who also voted for primary)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutCertificate {
    /// Slot that timed out
    pub slot: u64,
    
    /// Backup leader index
    pub backup_index: u32,
    
    /// Aggregated BLS signature from validators
    pub aggregated_signature: AggregatedBlsSignature,
    
    /// Timestamp when timeout occurred (Unix milliseconds)
    pub timeout_timestamp: i64,
    
    /// Minimum timeout duration that should have elapsed (milliseconds)
    pub min_timeout_ms: u64,
}

impl TimeoutCertificate {
    /// Create a new timeout certificate
    /// 
    /// # Arguments
    /// * `slot` - The slot that timed out
    /// * `backup_index` - Which backup leader this is for (1+)
    /// * `aggregated_signature` - Aggregated BLS signature from validators
    /// * `timeout_timestamp` - When the timeout occurred
    /// * `min_timeout_ms` - Minimum timeout that should have elapsed
    pub fn new(
        slot: u64,
        backup_index: u32,
        aggregated_signature: AggregatedBlsSignature,
        timeout_timestamp: i64,
        min_timeout_ms: u64,
    ) -> Self {
        Self {
            slot,
            backup_index,
            aggregated_signature,
            timeout_timestamp,
            min_timeout_ms,
        }
    }
    
    /// Validate this timeout certificate
    /// 
    /// # Arguments
    /// * `validators` - Current validator set with stake weights
    /// * `slot_start_timestamp` - When the slot started (Unix milliseconds)
    /// * `primary_voters` - Set of validators who voted for the primary leader's block
    /// 
    /// # Returns
    /// Ok(()) if valid, Err if invalid
    /// 
    /// # Validation Rules
    /// 1. Must have 2/3 stake signatures
    /// 2. Timeout must not be premature (elapsed time >= min_timeout_ms)
    /// 3. No conflicting votes (signers must not have voted for primary)
    pub fn validate(
        &self,
        validators: &[BlsValidatorInfo],
        slot_start_timestamp: i64,
        primary_voters: &HashSet<ValidatorID>,
    ) -> Result<()> {
        // Calculate total stake
        let total_stake: u64 = validators.iter().map(|v| v.stake).sum();
        
        // Verify 2/3 stake threshold
        if self.aggregated_signature.stake_weight * 3 < total_stake * 2 {
            return Err(BlockchainError::Consensus(format!(
                "Insufficient stake for timeout certificate: {} < 2/3 of {}",
                self.aggregated_signature.stake_weight,
                total_stake
            )));
        }
        
        // Verify timeout was not premature
        let elapsed_ms = self.timeout_timestamp - slot_start_timestamp;
        if elapsed_ms < self.min_timeout_ms as i64 {
            return Err(BlockchainError::Consensus(format!(
                "Premature timeout certificate: elapsed {}ms < minimum {}ms",
                elapsed_ms,
                self.min_timeout_ms
            )));
        }
        
        // Verify no conflicting votes
        for signer in &self.aggregated_signature.signers {
            if primary_voters.contains(signer) {
                return Err(BlockchainError::Consensus(format!(
                    "Conflicting vote: validator {:?} signed both timeout and primary block",
                    signer
                )));
            }
        }
        
        // Verify BLS signature
        let message = self.timeout_message();
        let aggregator = SignatureAggregator::new(validators.to_vec());
        let verified = aggregator.verify_aggregated(&message, &self.aggregated_signature)?;
        
        if !verified {
            return Err(BlockchainError::Consensus(
                "Invalid BLS signature on timeout certificate".to_string()
            ));
        }
        
        Ok(())
    }
    
    /// Generate the message that validators sign for timeout
    /// 
    /// Message format: "TIMEOUT:<slot>:<backup_index>"
    pub fn timeout_message(&self) -> Vec<u8> {
        format!("TIMEOUT:{}:{}", self.slot, self.backup_index).into_bytes()
    }
    
    /// Serialize for network transmission
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|e| {
            BlockchainError::Serialization(format!("Failed to serialize timeout certificate: {}", e))
        })
    }
    
    /// Deserialize from network
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        bincode::deserialize(data).map_err(|e| {
            BlockchainError::Serialization(format!("Failed to deserialize timeout certificate: {}", e))
        })
    }
}

/// Builder for creating timeout certificates
pub struct TimeoutCertificateBuilder {
    slot: u64,
    backup_index: u32,
    #[allow(dead_code)]
    validators: Vec<BlsValidatorInfo>,
    aggregator: SignatureAggregator,
    timeout_timestamp: i64,
    min_timeout_ms: u64,
}

impl TimeoutCertificateBuilder {
    /// Create a new builder
    pub fn new(
        slot: u64,
        backup_index: u32,
        validators: Vec<BlsValidatorInfo>,
        min_timeout_ms: u64,
    ) -> Self {
        let aggregator = SignatureAggregator::new(validators.clone());
        
        Self {
            slot,
            backup_index,
            validators,
            aggregator,
            timeout_timestamp: 0,
            min_timeout_ms,
        }
    }
    
    /// Add a timeout signature from a validator
    /// 
    /// # Returns
    /// Ok(true) if signature was valid and added
    /// Ok(false) if signature was invalid
    /// Err if validator is unknown
    pub fn add_signature(
        &mut self,
        validator_id: ValidatorID,
        signature: BlsSignature,
    ) -> Result<bool> {
        let message = self.timeout_message();
        self.aggregator.add_signature(validator_id, signature, &message)
    }
    
    /// Check if we have reached 2/3 threshold
    pub fn has_threshold(&self) -> bool {
        self.aggregator.has_threshold()
    }
    
    /// Set the timeout timestamp
    pub fn set_timeout_timestamp(&mut self, timestamp: i64) {
        self.timeout_timestamp = timestamp;
    }
    
    /// Try to build the timeout certificate if threshold is met
    pub fn try_build(&self) -> Option<TimeoutCertificate> {
        if !self.has_threshold() {
            return None;
        }
        
        let aggregated_signature = self.aggregator.try_aggregate()?;
        
        Some(TimeoutCertificate::new(
            self.slot,
            self.backup_index,
            aggregated_signature,
            self.timeout_timestamp,
            self.min_timeout_ms,
        ))
    }
    
    /// Generate the timeout message
    fn timeout_message(&self) -> Vec<u8> {
        format!("TIMEOUT:{}:{}", self.slot, self.backup_index).into_bytes()
    }
    
    /// Get current stake percentage
    pub fn stake_percentage(&self) -> f64 {
        self.aggregator.stake_percentage()
    }
}

/// Slashing evidence for premature timeout signatures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrematureTimeoutEvidence {
    /// Validator who signed prematurely
    pub validator_id: ValidatorID,
    
    /// The premature timeout certificate
    pub timeout_cert: TimeoutCertificate,
    
    /// Slot start timestamp
    pub slot_start_timestamp: i64,
    
    /// Proof that timeout was premature
    pub elapsed_ms: i64,
}

impl PrematureTimeoutEvidence {
    /// Create evidence of premature timeout
    pub fn new(
        validator_id: ValidatorID,
        timeout_cert: TimeoutCertificate,
        slot_start_timestamp: i64,
    ) -> Self {
        let elapsed_ms = timeout_cert.timeout_timestamp - slot_start_timestamp;
        
        Self {
            validator_id,
            timeout_cert,
            slot_start_timestamp,
            elapsed_ms,
        }
    }
    
    /// Verify this evidence is valid
    /// 
    /// Returns true if the validator did sign prematurely
    pub fn verify(&self) -> bool {
        // Check if validator is in the signers list
        if !self.timeout_cert.aggregated_signature.signers.contains(&self.validator_id) {
            return false;
        }
        
        // Check if timeout was premature
        self.elapsed_ms < self.timeout_cert.min_timeout_ms as i64
    }
    
    /// Calculate slashing amount (5% of stake)
    pub fn slashing_amount(&self, validator_stake: u64) -> u64 {
        validator_stake / 20 // 5%
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use blockchain_common::types::{BlockHeader, AggregatedSignature, BlockHash, MerkleRoot, StateRoot};
    
    fn create_test_block(slot: u64) -> Block {
        Block {
            header: BlockHeader {
                slot,
                height: slot,
                timestamp: 0,
                previous_hash: BlockHash([0u8; 32]),
                transactions_root: MerkleRoot([0u8; 32]),
                state_root: StateRoot([0u8; 32]),
                proposer: ValidatorID([0u8; 32]),
                epoch: 0,
                version: 1,
            },
            transactions: vec![],
            signature: AggregatedSignature {
                signature: vec![],
                signers: vec![],
            },
            parents: vec![],
        }
    }
    
    #[test]
    fn test_new_timeout_manager() {
        let manager = TimeoutManager::new(400);
        
        assert_eq!(manager.base_timeout(), Duration::from_millis(800));
        assert_eq!(manager.current_timeout(), Duration::from_millis(800));
        assert_eq!(manager.backup_index(), 0);
    }
    
    #[test]
    fn test_minimum_base_timeout() {
        // Even with low latency, base timeout should be at least 800ms
        let manager = TimeoutManager::new(100);
        
        assert_eq!(manager.base_timeout(), Duration::from_millis(800));
    }
    
    #[test]
    fn test_high_latency_base_timeout() {
        // With high latency, base timeout should be 2× p99
        let manager = TimeoutManager::new(500);
        
        assert_eq!(manager.base_timeout(), Duration::from_millis(1000));
    }
    
    #[test]
    fn test_escalation() {
        let mut manager = TimeoutManager::new(400);
        
        // Initial state
        assert_eq!(manager.backup_index(), 0);
        assert_eq!(manager.current_timeout(), Duration::from_millis(800));
        
        // First escalation
        manager.escalate();
        assert_eq!(manager.backup_index(), 1);
        assert_eq!(manager.current_timeout(), Duration::from_millis(1200)); // 800 * 1.5
        
        // Second escalation
        manager.escalate();
        assert_eq!(manager.backup_index(), 2);
        assert_eq!(manager.current_timeout(), Duration::from_millis(1800)); // 1200 * 1.5
        
        // Third escalation
        manager.escalate();
        assert_eq!(manager.backup_index(), 3);
        assert_eq!(manager.current_timeout(), Duration::from_millis(2700)); // 1800 * 1.5
    }
    
    #[test]
    fn test_reset() {
        let mut manager = TimeoutManager::new(400);
        
        // Escalate a few times
        manager.escalate();
        manager.escalate();
        
        assert_eq!(manager.backup_index(), 2);
        assert_ne!(manager.current_timeout(), manager.base_timeout());
        
        // Reset should restore initial state
        manager.reset();
        assert_eq!(manager.backup_index(), 0);
        assert_eq!(manager.current_timeout(), manager.base_timeout());
    }
    
    #[test]
    fn test_update_base_timeout() {
        let mut manager = TimeoutManager::new(400);
        
        assert_eq!(manager.base_timeout(), Duration::from_millis(800));
        
        // Update with higher latency
        manager.update_base_timeout(600);
        assert_eq!(manager.base_timeout(), Duration::from_millis(1200));
        assert_eq!(manager.current_timeout(), Duration::from_millis(1200));
        
        // Escalate and update - current timeout should not change
        manager.escalate();
        let escalated_timeout = manager.current_timeout();
        manager.update_base_timeout(500);
        assert_eq!(manager.current_timeout(), escalated_timeout);
    }
    
    #[test]
    fn test_wait_for_block_immediate_success() {
        let mut manager = TimeoutManager::new(400);
        let test_block = create_test_block(100);
        let test_block_clone = test_block.clone();
        
        let mut call_count = 0;
        let result = manager.wait_for_block(100, || {
            call_count += 1;
            Some(test_block_clone.clone())
        });
        
        match result {
            TimeoutResult::BlockReceived(block) => {
                assert_eq!(block.header.slot, 100);
            }
            TimeoutResult::Timeout => panic!("Should not timeout"),
        }
    }
    
    #[test]
    fn test_wait_for_block_timeout() {
        let mut manager = TimeoutManager::new(1); // Very short timeout for testing
        
        let result = manager.wait_for_block(100, || None);
        
        match result {
            TimeoutResult::BlockReceived(_) => panic!("Should timeout"),
            TimeoutResult::Timeout => {
                assert_eq!(manager.backup_index(), 1);
            }
        }
    }
    
    #[test]
    fn test_wait_for_block_wrong_slot() {
        let mut manager = TimeoutManager::new(1); // Very short timeout
        let wrong_block = create_test_block(99);
        
        let result = manager.wait_for_block(100, || Some(wrong_block.clone()));
        
        // Should timeout because block is for wrong slot
        match result {
            TimeoutResult::BlockReceived(_) => panic!("Should timeout due to wrong slot"),
            TimeoutResult::Timeout => {
                assert_eq!(manager.backup_index(), 1);
            }
        }
    }
    
    #[test]
    fn test_calculate_deadline() {
        let manager = TimeoutManager::new(400);
        
        let deadline = manager.calculate_deadline();
        let expected = Instant::now() + Duration::from_millis(800);
        
        // Allow small margin for test execution time
        let diff = if deadline > expected {
            deadline.duration_since(expected)
        } else {
            expected.duration_since(deadline)
        };
        
        assert!(diff < Duration::from_millis(10));
    }
    
    #[test]
    fn test_is_expired() {
        let manager = TimeoutManager::new(400);
        
        let past_deadline = Instant::now() - Duration::from_secs(1);
        assert!(manager.is_expired(past_deadline));
        
        let future_deadline = Instant::now() + Duration::from_secs(1);
        assert!(!manager.is_expired(future_deadline));
    }
    
    #[test]
    fn test_timeout_certificate_message() {
        use crate::bls::AggregatedBlsSignature;
        
        let cert = TimeoutCertificate::new(
            100,
            1,
            AggregatedBlsSignature::from_bytes(vec![0u8; 96], vec![], 0),
            1000,
            800,
        );
        
        let message = cert.timeout_message();
        assert_eq!(message, b"TIMEOUT:100:1");
    }
    
    #[test]
    fn test_timeout_certificate_validation_insufficient_stake() {
        use crate::bls::{BlsKeyPair, AggregatedBlsSignature, ValidatorInfo as BlsValidatorInfo};
        use std::collections::HashSet;
        
        // Create validators with total stake 300k
        let mut validators = Vec::new();
        for i in 0..3 {
            let keypair = BlsKeyPair::generate().unwrap();
            let validator_id = ValidatorID([i as u8; 32]);
            let validator_info = BlsValidatorInfo::new(
                validator_id,
                keypair.public_key_bytes(),
                100_000,
            );
            validators.push(validator_info);
        }
        
        // Create certificate with only 100k stake (33%, less than 2/3)
        let cert = TimeoutCertificate::new(
            100,
            1,
            AggregatedBlsSignature::from_bytes(
                vec![0u8; 96],
                vec![ValidatorID([0u8; 32])],
                100_000,
            ),
            2000,
            800,
        );
        
        let result = cert.validate(&validators, 1000, &HashSet::new());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Insufficient stake"));
    }
    
    #[test]
    fn test_timeout_certificate_validation_premature() {
        use crate::bls::{BlsKeyPair, AggregatedBlsSignature, ValidatorInfo as BlsValidatorInfo};
        use std::collections::HashSet;
        
        // Create validators
        let mut validators = Vec::new();
        for i in 0..3 {
            let keypair = BlsKeyPair::generate().unwrap();
            let validator_id = ValidatorID([i as u8; 32]);
            let validator_info = BlsValidatorInfo::new(
                validator_id,
                keypair.public_key_bytes(),
                100_000,
            );
            validators.push(validator_info);
        }
        
        // Create certificate with 200k stake (67%, exceeds 2/3) but premature timeout
        let cert = TimeoutCertificate::new(
            100,
            1,
            AggregatedBlsSignature::from_bytes(
                vec![0u8; 96],
                vec![ValidatorID([0u8; 32]), ValidatorID([1u8; 32])],
                200_000,
            ),
            1500, // Only 500ms elapsed
            800,  // Minimum 800ms required
        );
        
        let result = cert.validate(&validators, 1000, &HashSet::new());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Premature timeout"));
    }
    
    #[test]
    fn test_timeout_certificate_validation_conflicting_votes() {
        use crate::bls::{BlsKeyPair, AggregatedBlsSignature, ValidatorInfo as BlsValidatorInfo};
        use std::collections::HashSet;
        
        // Create validators
        let mut validators = Vec::new();
        for i in 0..3 {
            let keypair = BlsKeyPair::generate().unwrap();
            let validator_id = ValidatorID([i as u8; 32]);
            let validator_info = BlsValidatorInfo::new(
                validator_id,
                keypair.public_key_bytes(),
                100_000,
            );
            validators.push(validator_info);
        }
        
        // Create certificate with 200k stake
        let signer1 = ValidatorID([0u8; 32]);
        let signer2 = ValidatorID([1u8; 32]);
        
        let cert = TimeoutCertificate::new(
            100,
            1,
            AggregatedBlsSignature::from_bytes(
                vec![0u8; 96],
                vec![signer1, signer2],
                200_000,
            ),
            2000, // 1000ms elapsed, sufficient
            800,
        );
        
        // Create set of primary voters that includes one of the timeout signers
        let mut primary_voters = HashSet::new();
        primary_voters.insert(signer1);
        
        let result = cert.validate(&validators, 1000, &primary_voters);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Conflicting vote"));
    }
    
    #[test]
    fn test_timeout_certificate_builder() {
        use crate::bls::{BlsKeyPair, ValidatorInfo as BlsValidatorInfo};
        
        // Create validators and keypairs
        let mut validators = Vec::new();
        let mut keypairs = Vec::new();
        
        for i in 0..4 {
            let keypair = BlsKeyPair::generate().unwrap();
            let validator_id = ValidatorID([i as u8; 32]);
            let validator_info = BlsValidatorInfo::new(
                validator_id,
                keypair.public_key_bytes(),
                100_000,
            );
            validators.push(validator_info);
            keypairs.push(keypair);
        }
        
        let mut builder = TimeoutCertificateBuilder::new(100, 1, validators, 800);
        builder.set_timeout_timestamp(2000);
        
        // Add signatures from 3 validators (75% stake)
        for i in 0..3 {
            let message = builder.timeout_message();
            let signature = keypairs[i].sign(&message);
            let validator_id = ValidatorID([i as u8; 32]);
            let result = builder.add_signature(validator_id, signature).unwrap();
            assert!(result);
        }
        
        // Should have threshold
        assert!(builder.has_threshold());
        assert!(builder.stake_percentage() >= 66.0);
        
        // Should be able to build certificate
        let cert = builder.try_build();
        assert!(cert.is_some());
        
        let cert = cert.unwrap();
        assert_eq!(cert.slot, 100);
        assert_eq!(cert.backup_index, 1);
        assert_eq!(cert.aggregated_signature.signers.len(), 3);
    }
    
    #[test]
    fn test_premature_timeout_evidence() {
        use crate::bls::AggregatedBlsSignature;
        
        let validator_id = ValidatorID([0u8; 32]);
        let cert = TimeoutCertificate::new(
            100,
            1,
            AggregatedBlsSignature::from_bytes(
                vec![0u8; 96],
                vec![validator_id],
                100_000,
            ),
            1500, // Only 500ms elapsed
            800,  // Minimum 800ms required
        );
        
        let evidence = PrematureTimeoutEvidence::new(validator_id, cert, 1000);
        
        assert_eq!(evidence.elapsed_ms, 500);
        assert!(evidence.verify());
        
        // Calculate slashing (5% of 100k = 5k)
        assert_eq!(evidence.slashing_amount(100_000), 5_000);
    }
    
    #[test]
    fn test_premature_timeout_evidence_invalid_validator() {
        use crate::bls::AggregatedBlsSignature;
        
        let signer = ValidatorID([0u8; 32]);
        let non_signer = ValidatorID([1u8; 32]);
        
        let cert = TimeoutCertificate::new(
            100,
            1,
            AggregatedBlsSignature::from_bytes(
                vec![0u8; 96],
                vec![signer],
                100_000,
            ),
            1500,
            800,
        );
        
        // Create evidence for validator who didn't sign
        let evidence = PrematureTimeoutEvidence::new(non_signer, cert, 1000);
        
        // Should not verify because validator is not in signers list
        assert!(!evidence.verify());
    }
}
