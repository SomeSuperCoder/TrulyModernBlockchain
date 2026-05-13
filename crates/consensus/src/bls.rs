use blst::min_pk::{PublicKey, SecretKey, Signature, AggregateSignature};
use blst::BLST_ERROR;
use blockchain_common::{ValidatorID, BlockchainError};
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// BLS12-381 key pair for validator signing
#[derive(Clone)]
pub struct BlsKeyPair {
    secret_key: SecretKey,
    public_key: PublicKey,
}

impl BlsKeyPair {
    /// Generate a new random BLS key pair
    pub fn generate() -> Result<Self, BlockchainError> {
        let mut rng = rand::thread_rng();
        let mut ikm = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rng, &mut ikm);
        
        let secret_key = SecretKey::key_gen(&ikm, &[])
            .map_err(|e| BlockchainError::CryptoError(format!("Failed to generate secret key: {:?}", e)))?;
        let public_key = secret_key.sk_to_pk();
        
        Ok(Self {
            secret_key,
            public_key,
        })
    }
    
    /// Create key pair from existing secret key bytes
    pub fn from_bytes(secret_bytes: &[u8]) -> Result<Self, BlockchainError> {
        let secret_key = SecretKey::from_bytes(secret_bytes)
            .map_err(|e| BlockchainError::CryptoError(format!("Invalid secret key: {:?}", e)))?;
        let public_key = secret_key.sk_to_pk();
        
        Ok(Self {
            secret_key,
            public_key,
        })
    }
    
    /// Get the public key
    pub fn public_key(&self) -> &PublicKey {
        &self.public_key
    }
    
    /// Get the secret key bytes
    pub fn secret_key_bytes(&self) -> Vec<u8> {
        self.secret_key.to_bytes().to_vec()
    }
    
    /// Get the public key bytes (48 bytes for BLS12-381)
    pub fn public_key_bytes(&self) -> Vec<u8> {
        self.public_key.to_bytes().to_vec()
    }
    
    /// Sign a message with this key pair
    pub fn sign(&self, message: &[u8]) -> BlsSignature {
        let signature = self.secret_key.sign(message, &[], &[]);
        BlsSignature {
            signature,
        }
    }
    
    /// Verify a signature against this public key
    pub fn verify(&self, message: &[u8], signature: &BlsSignature) -> bool {
        let result = signature.signature.verify(true, message, &[], &[], &self.public_key, true);
        result == BLST_ERROR::BLST_SUCCESS
    }
}

/// BLS signature wrapper
#[derive(Clone)]
pub struct BlsSignature {
    signature: Signature,
}

impl BlsSignature {
    /// Create signature from bytes (96 bytes for BLS12-381)
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, BlockchainError> {
        let signature = Signature::from_bytes(bytes)
            .map_err(|e| BlockchainError::CryptoError(format!("Invalid signature: {:?}", e)))?;
        
        Ok(Self { signature })
    }
    
    /// Get signature bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        self.signature.to_bytes().to_vec()
    }
    
    /// Verify this signature against a public key and message
    pub fn verify(&self, message: &[u8], public_key: &PublicKey) -> bool {
        let result = self.signature.verify(true, message, &[], &[], public_key, true);
        result == BLST_ERROR::BLST_SUCCESS
    }
}

/// Validator information for signature aggregation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidatorInfo {
    /// Validator identifier
    pub id: ValidatorID,
    
    /// BLS public key bytes (48 bytes)
    pub public_key: Vec<u8>,
    
    /// Stake weight for this validator
    pub stake: u64,
}

impl ValidatorInfo {
    /// Create a new ValidatorInfo
    pub fn new(id: ValidatorID, public_key: Vec<u8>, stake: u64) -> Self {
        Self {
            id,
            public_key,
            stake,
        }
    }
    
    /// Get the BLS public key
    pub fn get_public_key(&self) -> Result<PublicKey, BlockchainError> {
        PublicKey::from_bytes(&self.public_key)
            .map_err(|e| BlockchainError::CryptoError(format!("Invalid public key: {:?}", e)))
    }
}

/// Aggregates BLS signatures from multiple validators with stake weighting
pub struct SignatureAggregator {
    /// Validator public keys and stake weights
    validators: HashMap<ValidatorID, ValidatorInfo>,
    
    /// Collected signatures for current message
    signatures: HashMap<ValidatorID, BlsSignature>,
    
    /// Total stake in the validator set
    total_stake: u64,
    
    /// Accumulated stake from signatures
    signed_stake: u64,
}

impl SignatureAggregator {
    /// Create a new signature aggregator with validator set
    pub fn new(validators: Vec<ValidatorInfo>) -> Self {
        let total_stake: u64 = validators.iter().map(|v| v.stake).sum();
        let validators_map: HashMap<ValidatorID, ValidatorInfo> = validators
            .into_iter()
            .map(|v| (v.id, v))
            .collect();
        
        Self {
            validators: validators_map,
            signatures: HashMap::new(),
            total_stake,
            signed_stake: 0,
        }
    }
    
    /// Add a signature from a validator
    /// Returns true if the signature is valid and was added
    pub fn add_signature(
        &mut self,
        validator_id: ValidatorID,
        signature: BlsSignature,
        message: &[u8],
    ) -> Result<bool, BlockchainError> {
        // Check if validator exists
        let validator = self.validators.get(&validator_id)
            .ok_or_else(|| BlockchainError::InvalidValidator(
                format!("Unknown validator: {:?}", validator_id)
            ))?;
        
        // Verify signature
        let public_key = validator.get_public_key()?;
        if !signature.verify(message, &public_key) {
            return Ok(false);
        }
        
        // Check if already signed
        if self.signatures.contains_key(&validator_id) {
            return Ok(false);
        }
        
        // Add signature and update stake
        self.signatures.insert(validator_id, signature);
        self.signed_stake += validator.stake;
        
        Ok(true)
    }
    
    /// Check if we have reached 2/3 stake threshold
    pub fn has_threshold(&self) -> bool {
        // Need 2/3 of total stake
        self.signed_stake * 3 >= self.total_stake * 2
    }
    
    /// Get the current percentage of stake that has signed
    pub fn stake_percentage(&self) -> f64 {
        if self.total_stake == 0 {
            0.0
        } else {
            (self.signed_stake as f64 / self.total_stake as f64) * 100.0
        }
    }
    
    /// Try to aggregate signatures if threshold is met
    /// Returns None if threshold not met, or the aggregated signature
    pub fn try_aggregate(&self) -> Option<AggregatedBlsSignature> {
        if !self.has_threshold() {
            return None;
        }
        
        // Aggregate all signatures
        let signatures: Vec<&Signature> = self.signatures
            .values()
            .map(|s| &s.signature)
            .collect();
        
        if signatures.is_empty() {
            return None;
        }
        
        let mut aggregate = AggregateSignature::from_signature(&signatures[0]);
        for sig in &signatures[1..] {
            match aggregate.add_signature(sig, true) {
                Ok(_) => {},
                Err(e) => {
                    tracing::error!("Failed to aggregate signature: {:?}", e);
                    return None;
                }
            }
        }
        
        let aggregated_sig = aggregate.to_signature();
        let signers: Vec<ValidatorID> = self.signatures.keys().copied().collect();
        
        Some(AggregatedBlsSignature {
            signature: aggregated_sig.to_bytes().to_vec(),
            signers,
            stake_weight: self.signed_stake,
        })
    }
    
    /// Verify an aggregated signature
    pub fn verify_aggregated(
        &self,
        message: &[u8],
        aggregated: &AggregatedBlsSignature,
    ) -> Result<bool, BlockchainError> {
        // Verify we have 2/3 stake
        if aggregated.stake_weight * 3 < self.total_stake * 2 {
            return Ok(false);
        }
        
        // Get all public keys for signers
        let mut public_keys = Vec::new();
        for signer_id in &aggregated.signers {
            let validator = self.validators.get(signer_id)
                .ok_or_else(|| BlockchainError::InvalidValidator(
                    format!("Unknown signer: {:?}", signer_id)
                ))?;
            public_keys.push(validator.get_public_key()?);
        }
        
        // Parse aggregated signature
        let signature = Signature::from_bytes(&aggregated.signature)
            .map_err(|e| BlockchainError::CryptoError(format!("Invalid aggregated signature: {:?}", e)))?;
        
        // For BLS signature aggregation, we need to verify each signature individually
        // or use fast aggregate verify if all signed the same message
        // Using fast aggregate verify for same message
        let pk_refs: Vec<&PublicKey> = public_keys.iter().collect();
        let result = signature.fast_aggregate_verify(true, message, &[], &pk_refs);
        
        Ok(result == BLST_ERROR::BLST_SUCCESS)
    }
    
    /// Reset the aggregator for a new message
    pub fn reset(&mut self) {
        self.signatures.clear();
        self.signed_stake = 0;
    }
    
    /// Get the number of signatures collected
    pub fn signature_count(&self) -> usize {
        self.signatures.len()
    }
    
    /// Get the list of validators who have signed
    pub fn signers(&self) -> Vec<ValidatorID> {
        self.signatures.keys().copied().collect()
    }
}

/// Aggregated BLS signature with metadata
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AggregatedBlsSignature {
    /// Aggregated signature bytes (96 bytes for BLS12-381)
    pub signature: Vec<u8>,
    
    /// Validator IDs that contributed to this signature
    pub signers: Vec<ValidatorID>,
    
    /// Total stake weight of signers
    pub stake_weight: u64,
}

impl AggregatedBlsSignature {
    /// Create from bytes
    pub fn from_bytes(signature: Vec<u8>, signers: Vec<ValidatorID>, stake_weight: u64) -> Self {
        Self {
            signature,
            signers,
            stake_weight,
        }
    }
    
    /// Get signature bytes
    pub fn signature_bytes(&self) -> &[u8] {
        &self.signature
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_bls_key_generation() {
        let keypair = BlsKeyPair::generate().unwrap();
        let pubkey_bytes = keypair.public_key_bytes();
        
        // BLS12-381 public keys are 48 bytes
        assert_eq!(pubkey_bytes.len(), 48);
    }
    
    #[test]
    fn test_bls_sign_and_verify() {
        let keypair = BlsKeyPair::generate().unwrap();
        let message = b"Hello, blockchain!";
        
        let signature = keypair.sign(message);
        assert!(keypair.verify(message, &signature));
        
        // Wrong message should fail
        let wrong_message = b"Wrong message";
        assert!(!keypair.verify(wrong_message, &signature));
    }
    
    #[test]
    fn test_signature_aggregation() {
        // Create 4 validators
        let mut validators = Vec::new();
        let mut keypairs = Vec::new();
        
        for i in 0..4 {
            let keypair = BlsKeyPair::generate().unwrap();
            let validator_id = ValidatorID([i as u8; 32]);
            let validator_info = ValidatorInfo::new(
                validator_id,
                keypair.public_key_bytes(),
                100_000, // Equal stake
            );
            validators.push(validator_info);
            keypairs.push(keypair);
        }
        
        let mut aggregator = SignatureAggregator::new(validators);
        let message = b"Block hash to sign";
        
        // Add signatures from 3 validators (75% stake, exceeds 2/3 threshold)
        for i in 0..3 {
            let signature = keypairs[i].sign(message);
            let validator_id = ValidatorID([i as u8; 32]);
            let result = aggregator.add_signature(validator_id, signature, message).unwrap();
            assert!(result);
        }
        
        // Should have threshold
        assert!(aggregator.has_threshold());
        assert_eq!(aggregator.signature_count(), 3);
        
        // Try to aggregate
        let aggregated = aggregator.try_aggregate().unwrap();
        assert_eq!(aggregated.signers.len(), 3);
        assert_eq!(aggregated.stake_weight, 300_000);
        
        // Verify aggregated signature
        let verified = aggregator.verify_aggregated(message, &aggregated).unwrap();
        assert!(verified);
    }
    
    #[test]
    fn test_insufficient_stake() {
        // Create 4 validators
        let mut validators = Vec::new();
        let mut keypairs = Vec::new();
        
        for i in 0..4 {
            let keypair = BlsKeyPair::generate().unwrap();
            let validator_id = ValidatorID([i as u8; 32]);
            let validator_info = ValidatorInfo::new(
                validator_id,
                keypair.public_key_bytes(),
                100_000,
            );
            validators.push(validator_info);
            keypairs.push(keypair);
        }
        
        let mut aggregator = SignatureAggregator::new(validators);
        let message = b"Block hash to sign";
        
        // Add signatures from only 2 validators (50% stake, below 2/3 threshold)
        for i in 0..2 {
            let signature = keypairs[i].sign(message);
            let validator_id = ValidatorID([i as u8; 32]);
            aggregator.add_signature(validator_id, signature, message).unwrap();
        }
        
        // Should not have threshold
        assert!(!aggregator.has_threshold());
        
        // Try to aggregate should return None
        assert!(aggregator.try_aggregate().is_none());
    }
    
    #[test]
    fn test_invalid_signature_rejected() {
        let keypair1 = BlsKeyPair::generate().unwrap();
        let keypair2 = BlsKeyPair::generate().unwrap();
        
        let validator_id = ValidatorID([1; 32]);
        let validator_info = ValidatorInfo::new(
            validator_id,
            keypair1.public_key_bytes(),
            100_000,
        );
        
        let mut aggregator = SignatureAggregator::new(vec![validator_info]);
        let message = b"Block hash to sign";
        
        // Sign with wrong key
        let wrong_signature = keypair2.sign(message);
        
        // Should fail verification
        let result = aggregator.add_signature(validator_id, wrong_signature, message).unwrap();
        assert!(!result);
        assert_eq!(aggregator.signature_count(), 0);
    }
}
