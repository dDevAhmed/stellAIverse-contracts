#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String, Symbol, Vec};
use stellai_lib::{admin, errors::ContractError, ADMIN_KEY};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WaitlistEntry {
    pub user: Address,
    pub position: u64,
    pub joined_at: u64,
    pub status: WaitlistStatus,
    pub credit_score_threshold: u32,
    pub notified_at: Option<u64>,
    pub granted_at: Option<u64>,
    pub metadata: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
#[repr(u32)]
pub enum WaitlistStatus {
    Pending = 0,
    Notified = 1,
    Granted = 2,
    Declined = 3,
    Removed = 4,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub struct WaitlistConfig {
    pub max_batch_size: u32,
    pub notification_period_days: u64,
    pub acceptance_period_days: u64,
    pub min_credit_score: u32,
    pub auto_advance_enabled: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NotificationRecord {
    pub user: Address,
    pub batch_id: u64,
    pub notified_at: u64,
    pub expires_at: u64,
    pub accepted: bool,
}

#[contract]
pub struct CreditScoringWaitlist;

#[contractimpl]
impl CreditScoringWaitlist {
    /// Initialize the credit scoring waitlist contract.
    pub fn init_contract(
        env: Env,
        admin_addr: Address,
        config: WaitlistConfig,
    ) -> Result<(), ContractError> {
        if env.storage().instance().has(&Symbol::new(&env, ADMIN_KEY)) {
            return Err(ContractError::AlreadyInitialized);
        }
        admin_addr.require_auth();

        // Validate configuration
        if config.max_batch_size == 0 || config.max_batch_size > 1000 {
            return Err(ContractError::InvalidAgentId);
        }
        if config.notification_period_days == 0 || config.acceptance_period_days == 0 {
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

        Ok(())
    }

    /// Join the credit scoring waitlist.
    pub fn join_waitlist(
        env: Env,
        user: Address,
        min_credit_score: u32,
        metadata: String,
    ) -> Result<u64, ContractError> {
        user.require_auth();

        // Check if already on waitlist
        let user_key = (Symbol::new(&env, "user"), user.clone());
        if env.storage().instance().has(&user_key) {
            return Err(ContractError::AlreadyInitialized);
        }

        // Validate metadata length
        if metadata.len() > 500 {
            return Err(ContractError::InvalidAgentId);
        }

        let config: WaitlistConfig = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, "config"))
            .ok_or(ContractError::NotInitialized)?;

        let waitlist_counter: u64 = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, "waitlist_counter"))
            .unwrap_or(0);
        let new_position = waitlist_counter + 1;

        let entry = WaitlistEntry {
            user: user.clone(),
            position: new_position,
            joined_at: env.ledger().timestamp(),
            status: WaitlistStatus::Pending,
            credit_score_threshold: min_credit_score.max(config.min_credit_score),
            notified_at: None,
            granted_at: None,
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
            (Symbol::new(&env, "waitlist"), Symbol::new(&env, "joined")),
            (user, new_position, min_credit_score),
        );

        Ok(new_position)
    }

    /// Create and send notifications for next batch of users (admin only).
    pub fn notify_next_batch(
        env: Env,
        admin: Address,
        batch_size: u32,
    ) -> Result<u64, ContractError> {
        admin::verify_admin(&env, &admin)?;

        let config: WaitlistConfig = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, "config"))
            .ok_or(ContractError::NotInitialized)?;

        let actual_batch_size = batch_size.min(config.max_batch_size);
        let batch_counter: u64 = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, "batch_counter"))
            .unwrap_or(0);
        let new_batch_id = batch_counter + 1;

        let waitlist_counter: u64 = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, "waitlist_counter"))
            .unwrap_or(0);
        let mut notified_count = 0u32;
        let now = env.ledger().timestamp();
        let expires_at = now + (config.acceptance_period_days * 24 * 60 * 60);

        // Find next pending users
        for position in 1..=waitlist_counter {
            if notified_count >= actual_batch_size {
                break;
            }

            let position_key = (Symbol::new(&env, "position"), position);
            if let Some(user) = env.storage().instance().get::<_, Address>(&position_key) {
                let user_key = (Symbol::new(&env, "user"), user.clone());
                if let Some(mut entry) = env.storage().instance().get::<_, WaitlistEntry>(&user_key)
                {
                    if entry.status == WaitlistStatus::Pending {
                        // Update entry status
                        entry.status = WaitlistStatus::Notified;
                        entry.notified_at = Some(now);
                        env.storage().instance().set(&user_key, &entry);

                        // Create notification record
                        let notification = NotificationRecord {
                            user: user.clone(),
                            batch_id: new_batch_id,
                            notified_at: now,
                            expires_at,
                            accepted: false,
                        };

                        let notification_key = (Symbol::new(&env, "notification"), user.clone());
                        env.storage()
                            .instance()
                            .set(&notification_key, &notification);

                        notified_count += 1;

                        env.events().publish(
                            (Symbol::new(&env, "waitlist"), Symbol::new(&env, "notified")),
                            (user, new_batch_id, position),
                        );
                    }
                }
            }
        }

        if notified_count > 0 {
            env.storage()
                .instance()
                .set(&Symbol::new(&env, "batch_counter"), &new_batch_id);
        }

        Ok(notified_count as u64)
    }

    /// Accept credit scoring access (user action).
    pub fn accept_access(env: Env, user: Address) -> Result<(), ContractError> {
        user.require_auth();

        let user_key = (Symbol::new(&env, "user"), user.clone());
        let mut entry: WaitlistEntry = env
            .storage()
            .instance()
            .get(&user_key)
            .ok_or(ContractError::InvalidAgentId)?;

        if entry.status != WaitlistStatus::Notified {
            return Err(ContractError::InvalidAgentId);
        }

        // Check if notification has expired
        let notification_key = (Symbol::new(&env, "notification"), user.clone());
        let notification: NotificationRecord = env
            .storage()
            .instance()
            .get(&notification_key)
            .ok_or(ContractError::InvalidAgentId)?;

        if env.ledger().timestamp() > notification.expires_at {
            return Err(ContractError::InvalidAgentId);
        }

        // Update entry
        entry.status = WaitlistStatus::Granted;
        entry.granted_at = Some(env.ledger().timestamp());
        env.storage().instance().set(&user_key, &entry);

        // Update notification
        let mut updated_notification = notification;
        updated_notification.accepted = true;
        env.storage()
            .instance()
            .set(&notification_key, &updated_notification);

        env.events().publish(
            (Symbol::new(&env, "waitlist"), Symbol::new(&env, "accepted")),
            (user, entry.position),
        );

        Ok(())
    }

    /// Decline credit scoring access (user action).
    pub fn decline_access(env: Env, user: Address) -> Result<(), ContractError> {
        user.require_auth();

        let user_key = (Symbol::new(&env, "user"), user.clone());
        let mut entry: WaitlistEntry = env
            .storage()
            .instance()
            .get(&user_key)
            .ok_or(ContractError::InvalidAgentId)?;

        if entry.status != WaitlistStatus::Notified {
            return Err(ContractError::InvalidAgentId);
        }

        entry.status = WaitlistStatus::Declined;
        env.storage().instance().set(&user_key, &entry);

        env.events().publish(
            (Symbol::new(&env, "waitlist"), Symbol::new(&env, "declined")),
            (user, entry.position),
        );

        Ok(())
    }

    /// Remove user from waitlist (admin only).
    pub fn remove_user(env: Env, admin: Address, user: Address) -> Result<(), ContractError> {
        admin::verify_admin(&env, &admin)?;

        let user_key = (Symbol::new(&env, "user"), user.clone());
        let mut entry: WaitlistEntry = env
            .storage()
            .instance()
            .get(&user_key)
            .ok_or(ContractError::InvalidAgentId)?;

        entry.status = WaitlistStatus::Removed;
        env.storage().instance().set(&user_key, &entry);

        env.events().publish(
            (Symbol::new(&env, "waitlist"), Symbol::new(&env, "removed")),
            (user, entry.position),
        );

        Ok(())
    }

    /// Get waitlist entry for a user.
    pub fn get_waitlist_entry(env: Env, user: Address) -> Result<WaitlistEntry, ContractError> {
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
    pub fn get_waitlist_stats(env: Env) -> Result<WaitlistStats, ContractError> {
        let waitlist_counter: u64 = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, "waitlist_counter"))
            .unwrap_or(0);
        let mut stats = WaitlistStats {
            total_joined: waitlist_counter,
            pending: 0,
            notified: 0,
            granted: 0,
            declined: 0,
            removed: 0,
        };

        // Count by status (simplified - in production, maintain counters)
        for position in 1..=waitlist_counter {
            let position_key = (Symbol::new(&env, "position"), position);
            if let Some(user) = env.storage().instance().get::<_, Address>(&position_key) {
                let user_key = (Symbol::new(&env, "user"), user);
                if let Some(entry) = env.storage().instance().get::<_, WaitlistEntry>(&user_key) {
                    match entry.status {
                        WaitlistStatus::Pending => stats.pending += 1,
                        WaitlistStatus::Notified => stats.notified += 1,
                        WaitlistStatus::Granted => stats.granted += 1,
                        WaitlistStatus::Declined => stats.declined += 1,
                        WaitlistStatus::Removed => stats.removed += 1,
                    }
                }
            }
        }

        Ok(stats)
    }

    /// Get next batch of pending users.
    pub fn get_next_pending_users(env: Env, limit: u32) -> Vec<WaitlistEntry> {
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
                if let Some(entry) = env.storage().instance().get::<_, WaitlistEntry>(&user_key) {
                    if entry.status == WaitlistStatus::Pending {
                        results.push_back(entry);
                    }
                }
            }
        }

        results
    }

    /// Get configuration.
    pub fn get_config(env: Env) -> Result<WaitlistConfig, ContractError> {
        env.storage()
            .instance()
            .get(&Symbol::new(&env, "config"))
            .ok_or(ContractError::NotInitialized)
    }
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WaitlistStats {
    pub total_joined: u64,
    pub pending: u64,
    pub notified: u64,
    pub granted: u64,
    pub declined: u64,
    pub removed: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn test_waitlist_flow() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let user1 = Address::generate(&env);
        let user2 = Address::generate(&env);

        let config = WaitlistConfig {
            max_batch_size: 10,
            notification_period_days: 7,
            acceptance_period_days: 3,
            min_credit_score: 600,
            auto_advance_enabled: true,
        };

        let contract_id = env.register(CreditScoringWaitlist, ());
        let client = CreditScoringWaitlistClient::new(&env, &contract_id);

        client.init_contract(&admin, &config);

        // Users join waitlist
        let pos1 = client.join_waitlist(&user1, &650, &String::from_str(&env, "User 1"));
        let pos2 = client.join_waitlist(&user2, &700, &String::from_str(&env, "User 2"));

        assert_eq!(pos1, 1);
        assert_eq!(pos2, 2);

        // Check entries
        let entry1 = client.get_waitlist_entry(&user1);
        assert_eq!(entry1.status, WaitlistStatus::Pending);
        assert_eq!(entry1.credit_score_threshold, 650);

        // Admin notifies next batch
        let notified = client.notify_next_batch(&admin, &5);
        assert_eq!(notified, 2);

        // Users accept access
        client.accept_access(&user1);
        client.accept_access(&user2);

        // Check final status
        let final_entry1 = client.get_waitlist_entry(&user1);
        assert_eq!(final_entry1.status, WaitlistStatus::Granted);

        // Check stats
        let stats = client.get_waitlist_stats();
        assert_eq!(stats.total_joined, 2);
        assert_eq!(stats.granted, 2);
    }
}
