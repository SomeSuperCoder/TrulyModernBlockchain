use blockchain_common::types::ValidatorID;
use blockchain_common::error::{BlockchainError, Result};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use serde::{Deserialize, Serialize};

/// Information about a validator including their stake weight
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorInfo {
    pub id: ValidatorID,
    pub stake: u64,
}

/// Leader schedule for an epoch with stake-weighted VRF selection
/// Uses deterministic PRNG (ChaCha20) for reproducible leader assignment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderSchedule {
    /// Validators and their stake weights
    validators: Vec<ValidatorInfo>,
    
    /// Slot assignments for this epoch (maps slot index to validator ID)
    schedule: Vec<ValidatorID>,
    
    /// VRF seed (typically last block hash from previous epoch)
    seed: [u8; 32],
    
    /// Total stake in the validator set
    total_stake: u64,
    
    /// Epoch length in slots
    epoch_length: u64,
}

impl LeaderSchedule {
    /// Compute leader schedule for an epoch using stake-weighted VRF
    /// 
    /// # Arguments
    /// * `validators` - List of validators with their stake weights
    /// * `seed` - Random seed (typically last block hash)
    /// * `epoch_length` - Number of slots in this epoch (default: 432,000)
    /// 
    /// # Requirements
    /// Implements Requirements 5.1, 13.2, 13.3:
    /// - Uses stake-weighted VRF for deterministic leader selection
    /// - Assigns slots proportionally to validator stake
    /// - Uses ChaCha20 PRNG seeded by last block hash
    pub fn compute_for_epoch(
        validators: Vec<ValidatorInfo>,
        seed: [u8; 32],
        epoch_length: u64,
    ) -> Result<Self> {
        if validators.is_empty() {
            return Err(BlockchainError::InvalidValidator(
                "Cannot create schedule with no validators".to_string()
            ));
        }
        
        // Calculate total stake
        let total_stake: u64 = validators.iter().map(|v| v.stake).sum();
        
        if total_stake == 0 {
            return Err(BlockchainError::InvalidValidator(
                "Total stake cannot be zero".to_string()
            ));
        }
        
        // Initialize deterministic PRNG with seed
        let mut rng = ChaCha20Rng::from_seed(seed);
        
        // Generate schedule for each slot using stake-weighted selection
        let mut schedule = Vec::with_capacity(epoch_length as usize);
        
        for _slot in 0..epoch_length {
            // Generate random number in range [0, total_stake)
            let target = rng.gen_range(0..total_stake);
            let mut accumulated = 0u64;
            
            // Find validator whose stake range contains the target
            let mut selected_validator = validators[0].id;
            for validator in &validators {
                accumulated += validator.stake;
                if target < accumulated {
                    selected_validator = validator.id;
                    break;
                }
            }
            
            schedule.push(selected_validator);
        }
        
        Ok(Self {
            validators,
            schedule,
            seed,
            total_stake,
            epoch_length,
        })
    }
    
    /// Get the primary leader for a specific slot
    /// 
    /// # Arguments
    /// * `slot` - Absolute slot number
    /// 
    /// # Returns
    /// ValidatorID of the primary leader for this slot
    pub fn get_leader(&self, slot: u64) -> ValidatorID {
        let index = (slot % self.epoch_length) as usize;
        self.schedule[index]
    }
    
    /// Get backup leaders for a specific slot
    /// 
    /// # Arguments
    /// * `slot` - Absolute slot number
    /// * `count` - Number of backup leaders to return
    /// 
    /// # Returns
    /// Vector of ValidatorIDs for backup leaders (excludes primary leader)
    /// 
    /// # Implementation
    /// Backup leaders are selected from subsequent slots in the schedule,
    /// skipping any that match the primary leader to ensure diversity
    pub fn get_backup_leaders(&self, slot: u64, count: usize) -> Vec<ValidatorID> {
        let primary = self.get_leader(slot);
        let mut backups = Vec::with_capacity(count);
        
        let mut offset = 1;
        while backups.len() < count && offset <= self.epoch_length {
            let backup_slot = slot + offset;
            let backup = self.get_leader(backup_slot);
            
            // Only add if different from primary leader
            if backup != primary && !backups.contains(&backup) {
                backups.push(backup);
            }
            
            offset += 1;
        }
        
        backups
    }
    
    /// Get the total stake in the validator set
    pub fn total_stake(&self) -> u64 {
        self.total_stake
    }
    
    /// Get the stake for a specific validator
    pub fn get_validator_stake(&self, validator_id: &ValidatorID) -> Option<u64> {
        self.validators
            .iter()
            .find(|v| v.id == *validator_id)
            .map(|v| v.stake)
    }
    
    /// Get all validators in this schedule
    pub fn validators(&self) -> &[ValidatorInfo] {
        &self.validators
    }
    
    /// Get the seed used for this schedule
    pub fn seed(&self) -> [u8; 32] {
        self.seed
    }
    
    /// Get the epoch length
    pub fn epoch_length(&self) -> u64 {
        self.epoch_length
    }
    
    /// Serialize the schedule for persistence to Epoch_State file
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|e| {
            BlockchainError::Serialization(format!("Failed to serialize leader schedule: {}", e))
        })
    }
    
    /// Deserialize the schedule from Epoch_State file
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        bincode::deserialize(data).map_err(|e| {
            BlockchainError::Serialization(format!("Failed to deserialize leader schedule: {}", e))
        })
    }
}

/// Validator stake information for epoch management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpochState {
    /// Current epoch number
    pub epoch: u64,
    
    /// Leader schedule for this epoch
    pub schedule: LeaderSchedule,
    
    /// Epoch start slot
    pub start_slot: u64,
    
    /// Epoch end slot
    pub end_slot: u64,
}

impl EpochState {
    /// Create a new epoch state
    pub fn new(
        epoch: u64,
        schedule: LeaderSchedule,
        start_slot: u64,
    ) -> Self {
        let epoch_length = schedule.epoch_length();
        Self {
            epoch,
            schedule,
            start_slot,
            end_slot: start_slot + epoch_length - 1,
        }
    }
    
    /// Check if a slot is within this epoch
    pub fn contains_slot(&self, slot: u64) -> bool {
        slot >= self.start_slot && slot <= self.end_slot
    }
    
    /// Serialize for persistence
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|e| {
            BlockchainError::Serialization(format!("Failed to serialize epoch state: {}", e))
        })
    }
    
    /// Deserialize from storage
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        bincode::deserialize(data).map_err(|e| {
            BlockchainError::Serialization(format!("Failed to deserialize epoch state: {}", e))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    
    fn create_test_validators() -> Vec<ValidatorInfo> {
        vec![
            ValidatorInfo {
                id: ValidatorID([1u8; 32]),
                stake: 100_000,
            },
            ValidatorInfo {
                id: ValidatorID([2u8; 32]),
                stake: 200_000,
            },
            ValidatorInfo {
                id: ValidatorID([3u8; 32]),
                stake: 300_000,
            },
        ]
    }
    
    #[test]
    fn test_compute_for_epoch() {
        let validators = create_test_validators();
        let seed = [42u8; 32];
        let epoch_length = 1000;
        
        let schedule = LeaderSchedule::compute_for_epoch(validators, seed, epoch_length)
            .expect("Failed to compute schedule");
        
        assert_eq!(schedule.epoch_length(), epoch_length);
        assert_eq!(schedule.total_stake(), 600_000);
        assert_eq!(schedule.schedule.len(), epoch_length as usize);
    }
    
    #[test]
    fn test_deterministic_schedule() {
        let validators = create_test_validators();
        let seed = [42u8; 32];
        let epoch_length = 100;
        
        let schedule1 = LeaderSchedule::compute_for_epoch(validators.clone(), seed, epoch_length)
            .expect("Failed to compute schedule 1");
        let schedule2 = LeaderSchedule::compute_for_epoch(validators, seed, epoch_length)
            .expect("Failed to compute schedule 2");
        
        // Same seed should produce same schedule
        for slot in 0..epoch_length {
            assert_eq!(schedule1.get_leader(slot), schedule2.get_leader(slot));
        }
    }
    
    #[test]
    fn test_get_leader() {
        let validators = create_test_validators();
        let seed = [42u8; 32];
        let epoch_length = 100;
        
        let schedule = LeaderSchedule::compute_for_epoch(validators, seed, epoch_length)
            .expect("Failed to compute schedule");
        
        // Test that get_leader wraps around correctly
        let leader_0 = schedule.get_leader(0);
        let leader_100 = schedule.get_leader(100);
        assert_eq!(leader_0, leader_100);
    }
    
    #[test]
    fn test_get_backup_leaders() {
        let validators = create_test_validators();
        let seed = [42u8; 32];
        let epoch_length = 100;
        
        let schedule = LeaderSchedule::compute_for_epoch(validators, seed, epoch_length)
            .expect("Failed to compute schedule");
        
        let primary = schedule.get_leader(0);
        let backups = schedule.get_backup_leaders(0, 3);
        
        // Backups should not include primary
        assert!(!backups.contains(&primary));
        
        // Should have requested number of backups (or less if not enough unique validators)
        assert!(backups.len() <= 3);
        
        // All backups should be unique
        for i in 0..backups.len() {
            for j in (i + 1)..backups.len() {
                assert_ne!(backups[i], backups[j]);
            }
        }
    }
    
    #[test]
    fn test_stake_weighted_distribution() {
        let validators = create_test_validators();
        let seed = [42u8; 32];
        let epoch_length = 10000;
        
        let schedule = LeaderSchedule::compute_for_epoch(validators.clone(), seed, epoch_length)
            .expect("Failed to compute schedule");
        
        // Count assignments for each validator
        let mut counts = HashMap::new();
        for slot in 0..epoch_length {
            let leader = schedule.get_leader(slot);
            *counts.entry(leader).or_insert(0) += 1;
        }
        
        // Verify distribution is roughly proportional to stake
        // Validator 1: 100k stake (16.67%)
        // Validator 2: 200k stake (33.33%)
        // Validator 3: 300k stake (50.00%)
        
        let count1 = counts.get(&ValidatorID([1u8; 32])).unwrap_or(&0);
        let count2 = counts.get(&ValidatorID([2u8; 32])).unwrap_or(&0);
        let count3 = counts.get(&ValidatorID([3u8; 32])).unwrap_or(&0);
        
        // Allow 5% margin of error
        let expected1 = (epoch_length as f64 * 0.1667) as u64;
        let expected2 = (epoch_length as f64 * 0.3333) as u64;
        let expected3 = (epoch_length as f64 * 0.5000) as u64;
        
        let margin = (epoch_length as f64 * 0.05) as u64;
        
        assert!((*count1 as i64 - expected1 as i64).abs() < margin as i64);
        assert!((*count2 as i64 - expected2 as i64).abs() < margin as i64);
        assert!((*count3 as i64 - expected3 as i64).abs() < margin as i64);
    }
    
    #[test]
    fn test_serialization() {
        let validators = create_test_validators();
        let seed = [42u8; 32];
        let epoch_length = 100;
        
        let schedule = LeaderSchedule::compute_for_epoch(validators, seed, epoch_length)
            .expect("Failed to compute schedule");
        
        let serialized = schedule.serialize().expect("Failed to serialize");
        let deserialized = LeaderSchedule::deserialize(&serialized)
            .expect("Failed to deserialize");
        
        assert_eq!(schedule.epoch_length(), deserialized.epoch_length());
        assert_eq!(schedule.total_stake(), deserialized.total_stake());
        assert_eq!(schedule.seed(), deserialized.seed());
    }
    
    #[test]
    fn test_epoch_state() {
        let validators = create_test_validators();
        let seed = [42u8; 32];
        let epoch_length = 100;
        
        let schedule = LeaderSchedule::compute_for_epoch(validators, seed, epoch_length)
            .expect("Failed to compute schedule");
        
        let epoch_state = EpochState::new(1, schedule, 1000);
        
        assert_eq!(epoch_state.epoch, 1);
        assert_eq!(epoch_state.start_slot, 1000);
        assert_eq!(epoch_state.end_slot, 1099);
        
        assert!(epoch_state.contains_slot(1000));
        assert!(epoch_state.contains_slot(1050));
        assert!(epoch_state.contains_slot(1099));
        assert!(!epoch_state.contains_slot(999));
        assert!(!epoch_state.contains_slot(1100));
    }
}
