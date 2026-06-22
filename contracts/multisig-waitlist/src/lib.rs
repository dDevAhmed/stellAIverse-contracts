#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String, Symbol, Vec};
use stellai_lib::{admin, errors::ContractError, ADMIN_KEY};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultisigWaitlistEntry {
    pub user: Address,
    pub position: u64,
    pub joined_at: u64,
    pub status: WaitlistStatus,
    pub required_signers: u32,
    pub wallet_name: String,
    pub use_case: String,
    pub granted_at: Option<u64>,
    pub wallet_address: Option<Address>,
    pub metadata: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
#[repr(u32)]
pub enum WaitlistStatus {
    Pending = 0,
    Approved = 1,
    Granted = 2,
    Declined = 3,
    Removed = 4,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub struct MultisigConfig {
    pub max_signers: u32,
    pub min_signers: u32,
    pub max_batch_size: u32,
    pub approval_required: bool,
    pub beta_release_limit: u32,
    pub auto_grant_enabled: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovalRecord {
    pub user: Address,
    pub approved_by: Address,
    pub approved_at: u64,
    pub batch_id: u64,
    pub notes: String,
}

#[contract]
pub struct MultisigWaitlist;

#[contractimpl]
impl MultisigWaitlist {
    /// Initialize the multi-sig waitlist contract.
    pub fn init_contract(
        env: Env,
        admin_addr: Address,
        config: MultisigConfig,
    ) -> Result<(), ContractError> {
        if env.storage().instance().has(&Symbol::new(&env, ADMIN_KEY)) {
            return Err(ContractError::AlreadyInitialized);
        }
        admin_addr.require_auth();

        // Validate configuration
        if config.min_signers == 0
            || config.max_signers < config.min_signers
            || config.max_signers > 100
        {
            return Err(ContractError::InvalidAgentId);
        }
        if config.max_batch_size == 0 || config.max_batch_size > 500 {
            return Err(ContractError::InvalidAgentId);
        }

        env.storage()
            .instance()
            .set(&Symbol::new(&env, ADMIN_KEY), &admin_addr);
        env.storage()
            .instance()
            .set(&Symbol::new(&env, "config"), &config);
        env.storage()
            .instance()
            .set(&Symbol::new(&env, "waitlist_counter"), &0u64);
        env.storage()
            .instance()
            .set(&Symbol::new(&env, "batch_counter"), &0u64);
        env.storage()
            .instance()
            .set(&Symbol::new(&env, "granted_counter"), &0u64);

        Ok(())
    }

    /// Join the multi-sig waitlist.
    pub fn join_waitlist(
        env: Env,
        user: Address,
        required_signers: u32,
        wallet_name: String,
        use_case: String,
        metadata: String,
    ) -> Result<u64, ContractError> {
        user.require_auth();

        // Check if already on waitlist
        let user_key = (Symbol::new(&env, "user"), user.clone());
        if env.storage().instance().has(&user_key) {
            return Err(ContractError::AlreadyInitialized);
        }

        // Validate inputs
        if wallet_name.len() > 100 || use_case.len() > 500 || metadata.len() > 1000 {
            return Err(ContractError::InvalidAgentId);
        }

        let config: MultisigConfig = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, "config"))
            .ok_or(ContractError::NotInitialized)?;

        if required_signers < config.min_signers || required_signers > config.max_signers {
            return Err(ContractError::InvalidAgentId);
        }

        let waitlist_counter: u64 = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, "waitlist_counter"))
            .unwrap_or(0);
        let new_position = waitlist_counter + 1;

        let entry = MultisigWaitlistEntry {
            user: user.clone(),
            position: new_position,
            joined_at: env.ledger().timestamp(),
            status: if config.auto_grant_enabled {
                WaitlistStatus::Granted
            } else {
                WaitlistStatus::Pending
            },
            required_signers,
            wallet_name: wallet_name.clone(),
            use_case: use_case.clone(),
            granted_at: if config.auto_grant_enabled {
                Some(env.ledger().timestamp())
            } else {
                None
            },
            wallet_address: None,
            metadata: metadata.clone(),
        };

        env.storage()
            .instance()
            .set(&Symbol::new(&env, "waitlist_counter"), &new_position);
        env.storage().instance().set(&user_key, &entry);

        // Add to position index
        let position_key = (Symbol::new(&env, "position"), new_position);
        env.storage().instance().set(&position_key, &user);

        env.events().publish(
            (
                Symbol::new(&env, "multisig_waitlist"),
                Symbol::new(&env, "joined"),
            ),
            (user, new_position, required_signers),
        );

        Ok(new_position)
    }

    /// Approve users for multi-sig access (admin only).
    pub fn approve_users(
        env: Env,
        admin: Address,
        users: Vec<Address>,
        notes: String,
    ) -> Result<u64, ContractError> {
        admin::verify_admin(&env, &admin)?;

        let config: MultisigConfig = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, "config"))
            .ok_or(ContractError::NotInitialized)?;

        let batch_counter: u64 = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, "batch_counter"))
            .unwrap_or(0);
        let new_batch_id = batch_counter + 1;

        let granted_counter: u64 = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, "granted_counter"))
            .unwrap_or(0);
        let mut approved_count = 0u32;

        for user in users.iter() {
            if approved_count >= config.max_batch_size {
                break;
            }

            let user_key = (Symbol::new(&env, "user"), user.clone());
            if let Some(mut entry) = env
                .storage()
                .instance()
                .get::<_, MultisigWaitlistEntry>(&user_key)
            {
                if entry.status == WaitlistStatus::Pending {
                    // Check beta release limit
                    if granted_counter + approved_count as u64 >= config.beta_release_limit as u64 {
                        break;
                    }

                    entry.status = WaitlistStatus::Approved;
                    env.storage().instance().set(&user_key, &entry);

                    // Create approval record
                    let approval = ApprovalRecord {
                        user: user.clone(),
                        approved_by: admin.clone(),
                        approved_at: env.ledger().timestamp(),
                        batch_id: new_batch_id,
                        notes: notes.clone(),
                    };

                    let approval_key = (Symbol::new(&env, "approval"), user.clone());
                    env.storage().instance().set(&approval_key, &approval);

                    approved_count += 1;

                    env.events().publish(
                        (
                            Symbol::new(&env, "multisig_waitlist"),
                            Symbol::new(&env, "approved"),
                        ),
                        (user, new_batch_id),
                    );
                }
            }
        }

        if approved_count > 0 {
            env.storage()
                .instance()
                .set(&Symbol::new(&env, "batch_counter"), &new_batch_id);
        }

        Ok(approved_count as u64)
    }

    /// Grant multi-sig wallet access (admin only).
    pub fn grant_access(
        env: Env,
        admin: Address,
        user: Address,
        wallet_address: Address,
    ) -> Result<(), ContractError> {
        admin::verify_admin(&env, &admin)?;

        let user_key = (Symbol::new(&env, "user"), user.clone());
        let mut entry: MultisigWaitlistEntry = env
            .storage()
            .instance()
            .get(&user_key)
            .ok_or(ContractError::InvalidAgentId)?;

        if entry.status != WaitlistStatus::Approved {
            return Err(ContractError::InvalidAgentId);
        }

        let config: MultisigConfig = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, "config"))
            .ok_or(ContractError::NotInitialized)?;

        let granted_counter: u64 = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, "granted_counter"))
            .unwrap_or(0);
        if granted_counter >= config.beta_release_limit as u64 {
            return Err(ContractError::InvalidAgentId);
        }

        entry.status = WaitlistStatus::Granted;
        entry.granted_at = Some(env.ledger().timestamp());
        entry.wallet_address = Some(wallet_address.clone());
        env.storage().instance().set(&user_key, &entry);

        // Update granted counter
        env.storage().instance().set(
            &Symbol::new(&env, "granted_counter"),
            &(granted_counter + 1),
        );

        env.events().publish(
            (
                Symbol::new(&env, "multisig_waitlist"),
                Symbol::new(&env, "granted"),
            ),
            (user, wallet_address),
        );

        Ok(())
    }

    /// Decline multi-sig access (user action).
    pub fn decline_access(env: Env, user: Address) -> Result<(), ContractError> {
        user.require_auth();

        let user_key = (Symbol::new(&env, "user"), user.clone());
        let mut entry: MultisigWaitlistEntry = env
            .storage()
            .instance()
            .get(&user_key)
            .ok_or(ContractError::InvalidAgentId)?;

        if entry.status != WaitlistStatus::Approved {
            return Err(ContractError::InvalidAgentId);
        }

        entry.status = WaitlistStatus::Declined;
        env.storage().instance().set(&user_key, &entry);

        env.events().publish(
            (
                Symbol::new(&env, "multisig_waitlist"),
                Symbol::new(&env, "declined"),
            ),
            (user, entry.position),
        );

        Ok(())
    }

    /// Remove user from waitlist (admin only).
    pub fn remove_user(env: Env, admin: Address, user: Address) -> Result<(), ContractError> {
        admin::verify_admin(&env, &admin)?;

        let user_key = (Symbol::new(&env, "user"), user.clone());
        let mut entry: MultisigWaitlistEntry = env
            .storage()
            .instance()
            .get(&user_key)
            .ok_or(ContractError::InvalidAgentId)?;

        entry.status = WaitlistStatus::Removed;
        env.storage().instance().set(&user_key, &entry);

        env.events().publish(
            (
                Symbol::new(&env, "multisig_waitlist"),
                Symbol::new(&env, "removed"),
            ),
            (user, entry.position),
        );

        Ok(())
    }

    /// Get waitlist entry for a user.
    pub fn get_waitlist_entry(
        env: Env,
        user: Address,
    ) -> Result<MultisigWaitlistEntry, ContractError> {
        let key = (Symbol::new(&env, "user"), user);
        env.storage()
            .instance()
            .get(&key)
            .ok_or(ContractError::InvalidAgentId)
    }

    /// Get waitlist position for a user.
    pub fn get_waitlist_position(env: Env, user: Address) -> Result<u64, ContractError> {
        let entry = Self::get_waitlist_entry(env, user)?;
        Ok(entry.position)
    }

    /// Get waitlist statistics.
    pub fn get_waitlist_stats(env: Env) -> Result<MultisigWaitlistStats, ContractError> {
        let waitlist_counter: u64 = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, "waitlist_counter"))
            .unwrap_or(0);
        let granted_counter: u64 = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, "granted_counter"))
            .unwrap_or(0);
        let mut stats = MultisigWaitlistStats {
            total_joined: waitlist_counter,
            pending: 0,
            approved: 0,
            granted: granted_counter,
            declined: 0,
            removed: 0,
        };

        // Count by status (simplified - in production, maintain counters)
        for position in 1..=waitlist_counter {
            let position_key = (Symbol::new(&env, "position"), position);
            if let Some(user) = env.storage().instance().get::<_, Address>(&position_key) {
                let user_key = (Symbol::new(&env, "user"), user);
                if let Some(entry) = env
                    .storage()
                    .instance()
                    .get::<_, MultisigWaitlistEntry>(&user_key)
                {
                    match entry.status {
                        WaitlistStatus::Pending => stats.pending += 1,
                        WaitlistStatus::Approved => stats.approved += 1,
                        WaitlistStatus::Granted => stats.granted += 1,
                        WaitlistStatus::Declined => stats.declined += 1,
                        WaitlistStatus::Removed => stats.removed += 1,
                    }
                }
            }
        }

        Ok(stats)
    }

    /// Get next batch of pending users for approval.
    pub fn get_next_pending_users(env: Env, limit: u32) -> Vec<MultisigWaitlistEntry> {
        let waitlist_counter: u64 = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, "waitlist_counter"))
            .unwrap_or(0);
        let mut results = Vec::new(&env);

        for position in 1..=waitlist_counter {
            if results.len() >= limit as u32 {
                break;
            }

            let position_key = (Symbol::new(&env, "position"), position);
            if let Some(user) = env.storage().instance().get::<_, Address>(&position_key) {
                let user_key = (Symbol::new(&env, "user"), user);
                if let Some(entry) = env
                    .storage()
                    .instance()
                    .get::<_, MultisigWaitlistEntry>(&user_key)
                {
                    if entry.status == WaitlistStatus::Pending {
                        results.push_back(entry);
                    }
                }
            }
        }

        results
    }

    /// Get approved users awaiting wallet deployment.
    pub fn get_approved_users(env: Env, limit: u32) -> Vec<MultisigWaitlistEntry> {
        let waitlist_counter: u64 = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, "waitlist_counter"))
            .unwrap_or(0);
        let mut results = Vec::new(&env);

        for position in 1..=waitlist_counter {
            if results.len() >= limit as u32 {
                break;
            }

            let position_key = (Symbol::new(&env, "position"), position);
            if let Some(user) = env.storage().instance().get::<_, Address>(&position_key) {
                let user_key = (Symbol::new(&env, "user"), user);
                if let Some(entry) = env
                    .storage()
                    .instance()
                    .get::<_, MultisigWaitlistEntry>(&user_key)
                {
                    if entry.status == WaitlistStatus::Approved {
                        results.push_back(entry);
                    }
                }
            }
        }

        results
    }

    /// Get configuration.
    pub fn get_config(env: Env) -> Result<MultisigConfig, ContractError> {
        env.storage()
            .instance()
            .get(&Symbol::new(&env, "config"))
            .ok_or(ContractError::NotInitialized)
    }

    /// Check if beta limit is reached.
    pub fn is_beta_limit_reached(env: Env) -> Result<bool, ContractError> {
        let config: MultisigConfig = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, "config"))
            .ok_or(ContractError::NotInitialized)?;

        let granted_counter: u64 = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, "granted_counter"))
            .unwrap_or(0);

        Ok(granted_counter >= config.beta_release_limit as u64)
    }
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultisigWaitlistStats {
    pub total_joined: u64,
    pub pending: u64,
    pub approved: u64,
    pub granted: u64,
    pub declined: u64,
    pub removed: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn test_multisig_waitlist_flow() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);
        let wallet_addr = Address::generate(&env);

        let config = MultisigConfig {
            max_signers: 10,
            min_signers: 2,
            max_batch_size: 5,
            approval_required: true,
            beta_release_limit: 100,
            auto_grant_enabled: false,
        };

        let contract_id = env.register_contract(None, MultisigWaitlist);
        let client = MultisigWaitlistClient::new(&env, &contract_id);

        client.init_contract(&admin, &config);

        // Users join waitlist
        let pos1 = client.join_waitlist(
            &user1,
            &3,
            &String::from_str(&env, "Team Wallet"),
            &String::from_str(&env, "Team operations"),
            &String::from_str(&env, "Multi-sig for team"),
        );
        let pos2 = client.join_waitlist(
            &user2,
            &5,
            &String::from_str(&env, "DAO Treasury"),
            &String::from_str(&env, "DAO treasury management"),
            &String::from_str(&env, "DAO multi-sig"),
        );

        assert_eq!(pos1, 1);
        assert_eq!(pos2, 2);

        // Check entries
        let entry1 = client.get_waitlist_entry(&user1);
        assert_eq!(entry1.status, WaitlistStatus::Pending);
        assert_eq!(entry1.required_signers, 3);

        // Admin approves users
        let mut users = Vec::new(&env);
        users.push_back(user1.clone());
        users.push_back(user2.clone());

        let approved =
            client.approve_users(&admin, users, &String::from_str(&env, "Batch 1 approval"));
        assert_eq!(approved, 2);

        // Grant access to user1
        client.grant_access(&admin, &user1, &wallet_addr);

        // Check final status
        let final_entry1 = client.get_waitlist_entry(&user1);
        assert_eq!(final_entry1.status, WaitlistStatus::Granted);
        assert_eq!(final_entry1.wallet_address, Some(wallet_addr));

        // Check stats
        let stats = client.get_waitlist_stats();
        assert_eq!(stats.total_joined, 2);
        assert_eq!(stats.granted, 1);
        assert_eq!(stats.approved, 1);

        // Check beta limit
        assert!(!client.is_beta_limit_reached());
    }
}
