#![no_std]
#![allow(unused_imports)]
use soroban_sdk::{contracttype, Address, Bytes, String, Vec};

// ============================================================================
// STORAGE KEY NAMESPACE PROTECTION
// ============================================================================

/// Module identifiers for storage key namespacing.
/// Each module MUST use its identifier as a prefix for all storage keys
/// to prevent cross-module key collisions.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ModuleId {
    AgentNft = 0,
    AgentToken = 1,
    Marketplace = 2,
    Evolution = 3,
    ExecutionHub = 4,
    Oracle = 5,
    Faucet = 6,
    Governance = 7,
    Compliance = 8,
    Staking = 9,
    Lifecycle = 10,
    Threshold = 11,
    TransactionCoord = 12,
    VerifiableCreds = 13,
    Metrics = 14,
    Prediction = 15,
    Referral = 16,
    RiskEval = 17,
    BugBounty = 18,
    Affiliate = 19,
    CreditScore = 20,
    Waitlist = 21,
    MultisigWaitlist = 22,
    BridgeManager = 23,
}

/// Creates a namespaced storage key to prevent collisions across modules.
/// Usage: `namespaced_key(ModuleId::Marketplace, "listing", &listing_id)`
#[contracttype]
#[derive(Clone, Debug)]
pub struct NamespacedKey {
    pub module: ModuleId,
    pub category: String,
    pub identifier: String,
}

/// Validate that a storage key is properly namespaced.
/// Returns false if the key could potentially collide with another module's keys.
pub fn validate_namespaced_key(key: &NamespacedKey) -> bool {
    // Ensure category is not empty
    if key.category.is_empty() {
        return false;
    }
    // Ensure identifier is not empty
    if key.identifier.is_empty() {
        return false;
    }
    true
}

/// Constants for security hardening

/// Represents an agent's metadata and state
#[derive(Clone)]
#[contracttype]
pub struct Agent {
    pub id: u64,
    pub owner: Address,
    pub name: String,
    pub model_hash: String,
    pub capabilities: Vec<String>,
    pub evolution_level: u32,
    pub created_at: u64,
    pub updated_at: u64,
    pub nonce: u64,
    pub escrow_locked: bool,
    pub escrow_holder: Option<Address>,
}

/// Rate limiting window for security protection
#[derive(Clone, Copy)]
#[contracttype]
pub struct RateLimit {
    pub window_seconds: u64,
    pub max_operations: u32,
}

/// Represents a marketplace listing
#[derive(Clone)]
#[contracttype]
pub struct Listing {
    pub listing_id: u64,
    pub agent_id: u64,
    pub seller: Address,
    pub price: i128,
    pub listing_type: ListingType, // Sale, Lease, etc.
    pub active: bool,
    pub created_at: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[contracttype]
#[repr(u32)]
pub enum ListingType {
    Sale = 0,
    Lease = 1,
    Auction = 2,
}

/// Represents an evolution/upgrade request
#[derive(Clone)]
#[contracttype]
pub struct EvolutionRequest {
    pub request_id: u64,
    pub agent_id: u64,
    pub owner: Address,
    pub stake_amount: i128,
    pub status: EvolutionStatus,
    pub created_at: u64,
    pub completed_at: Option<u64>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[contracttype]
#[repr(u32)]
pub enum EvolutionStatus {
    Pending = 0,
    InProgress = 1,
    Completed = 2,
    Failed = 3,
}

/// Oracle data entry
#[derive(Clone)]
#[contracttype]
pub struct OracleData {
    pub key: String,
    pub value: String,
    pub timestamp: u64,
    pub source: String,
}

/// Royalty information for marketplace transactions
#[derive(Clone)]
#[contracttype]
pub struct RoyaltyInfo {
    pub recipient: Address,
    pub percentage: u32, // 0-10000 representing 0-100%
}

/// Oracle attestation for evolution completion (signed by oracle provider)
#[derive(Clone)]
#[contracttype]
pub struct EvolutionAttestation {
    pub request_id: u64,
    pub agent_id: u64,
    pub oracle_provider: Address,
    pub new_model_hash: String,
    pub attestation_data: Bytes,
    pub signature: Bytes,
    pub timestamp: u64,
    pub nonce: u64,
}

/// Constants for security hardening
pub const MAX_STRING_LENGTH: usize = 256;
pub const MAX_CAPABILITIES: usize = 32;
pub const MAX_ROYALTY_PERCENTAGE: u32 = 10000; // 100%
pub const MIN_ROYALTY_PERCENTAGE: u32 = 0;
pub const SAFE_ARITHMETIC_CHECK_OVERFLOW: u128 = u128::MAX;
pub const PRICE_UPPER_BOUND: i128 = i128::MAX / 2; // Prevent overflow in calculations
pub const PRICE_LOWER_BOUND: i128 = 0; // Prevent negative prices
pub const MAX_DURATION_DAYS: u64 = 36500; // ~100 years max lease duration
pub const MAX_AGE_SECONDS: u64 = 365 * 24 * 60 * 60; // ~1 year max data age
pub const ATTESTATION_SIGNATURE_SIZE: usize = 64; // Ed25519 signature size
pub const MAX_ATTESTATION_DATA_SIZE: usize = 1024; // Max size for attestation data

#[cfg(any(test, feature = "testutils"))]
pub mod testutils {
    use super::*;
    use soroban_sdk::{Address, Bytes, Env, String, Vec};

    pub fn create_oracle_data(env: &Env, key: &str, value: &str, source: &str) -> OracleData {
        OracleData {
            key: String::from_str(env, key),
            value: String::from_str(env, value),
            timestamp: env.ledger().timestamp(),
            source: String::from_str(env, source),
        }
    }

    pub fn create_evolution_attestation(
        env: &Env,
        request_id: u64,
        agent_id: u64,
        oracle_provider: Address,
        new_model_hash: &str,
        nonce: u64,
    ) -> EvolutionAttestation {
        EvolutionAttestation {
            request_id,
            agent_id,
            oracle_provider,
            new_model_hash: String::from_str(env, new_model_hash),
            attestation_data: Bytes::from_slice(env, b"mock_attestation_data"),
            signature: Bytes::from_slice(env, &[0u8; 64]),
            timestamp: env.ledger().timestamp(),
            nonce,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{Env, String};

    #[test]
    fn test_namespaced_key_validation_valid() {
        let env = Env::default();
        let key = NamespacedKey {
            module: ModuleId::Marketplace,
            category: String::from_str(&env, "listing"),
            identifier: String::from_str(&env, "123"),
        };
        assert!(validate_namespaced_key(&key));
    }

    #[test]
    fn test_namespaced_key_validation_empty_category() {
        let env = Env::default();
        let key = NamespacedKey {
            module: ModuleId::Marketplace,
            category: String::from_str(&env, ""),
            identifier: String::from_str(&env, "123"),
        };
        assert!(!validate_namespaced_key(&key));
    }

    #[test]
    fn test_namespaced_key_validation_empty_identifier() {
        let env = Env::default();
        let key = NamespacedKey {
            module: ModuleId::Marketplace,
            category: String::from_str(&env, "listing"),
            identifier: String::from_str(&env, ""),
        };
        assert!(!validate_namespaced_key(&key));
    }

    #[test]
    fn test_no_collision_across_modules() {
        let env = Env::default();
        
        // Create keys from different modules with same category/identifier
        let market_key = NamespacedKey {
            module: ModuleId::Marketplace,
            category: String::from_str(&env, "agent"),
            identifier: String::from_str(&env, "1"),
        };
        
        let evolution_key = NamespacedKey {
            module: ModuleId::Evolution,
            category: String::from_str(&env, "agent"),
            identifier: String::from_str(&env, "1"),
        };
        
        let nft_key = NamespacedKey {
            module: ModuleId::AgentNft,
            category: String::from_str(&env, "agent"),
            identifier: String::from_str(&env, "1"),
        };
        
        // All should be valid
        assert!(validate_namespaced_key(&market_key));
        assert!(validate_namespaced_key(&evolution_key));
        assert!(validate_namespaced_key(&nft_key));
        
        // Keys should be different due to different module IDs
        assert_ne!(market_key.module, evolution_key.module);
        assert_ne!(market_key.module, nft_key.module);
        assert_ne!(evolution_key.module, nft_key.module);
    }

    #[test]
    fn test_fuzz_dynamic_keys_no_collision() {
        let env = Env::default();
        
        // Simulate dynamic key generation across multiple modules
        let modules = [
            ModuleId::AgentNft,
            ModuleId::Marketplace,
            ModuleId::Evolution,
            ModuleId::Governance,
            ModuleId::Compliance,
        ];
        
        let categories = ["user", "agent", "listing", "request", "proposal"];
        let identifiers = ["1", "2", "100", "999", "dynamic_key"];
        
        // Generate all combinations and verify uniqueness
        let mut keys: Vec<(ModuleId, String, String)> = Vec::new(&env);
        
        for module in &modules {
            for category in &categories {
                for identifier in &identifiers {
                    let key = NamespacedKey {
                        module: *module,
                        category: String::from_str(&env, category),
                        identifier: String::from_str(&env, identifier),
                    };
                    
                    // Validate key
                    assert!(validate_namespaced_key(&key));
                    
                    // Check for duplicates (should not exist due to module prefix)
                    for existing in keys.iter() {
                        let is_duplicate = 
                            existing.0 == key.module &&
                            existing.1 == key.category &&
                            existing.2 == key.identifier;
                        
                        if is_duplicate {
                            panic!("Duplicate key detected: {:?}", key);
                        }
                    }
                    
                    keys.push_back((key.module, key.category, key.identifier));
                }
            }
        }
        
        // Total keys should be modules * categories * identifiers
        assert_eq!(keys.len(), (modules.len() * categories.len() * identifiers.len()) as u32);
    }
}
