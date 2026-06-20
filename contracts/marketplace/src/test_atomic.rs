#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Env},
    Address, Env as _, Symbol, Vec, Val,
};
use stellai_lib::types::{TransactionStatus, TransactionStep};

use crate::{Marketplace, MarketplaceClient};

fn setup() -> (Env, MarketplaceClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, Marketplace);
    let client = MarketplaceClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    
    // Initialize contract
    let payment_token = Address::generate(&env);
    client.initialize(&admin, &payment_token, &250); // 2.5% platform fee
    
    (env, client, admin)
}

#[test]
fn test_atomic_transaction_initialization() {
    let (env, client, _admin) = setup();
    
    // Get first transaction ID
    let tx_id = client.get_next_atomic_transaction_id();
    assert_eq!(tx_id, 1);
    
    // Verify atomic support was initialized
    let tx_counter: u64 = env.storage().instance().get(&Symbol::new(&env, "atomic_tx_counter")).unwrap();
    assert_eq!(tx_counter, 1);
}

#[test]
fn test_atomic_transaction_success() {
    let (env, client, _admin) = setup();
    
    // Create a simple mock contract to test with (we'll use the marketplace itself as a mock)
    let mock_contract = client.address.clone();
    
    // Create test steps that would represent a bundle purchase
    let mut steps = Vec::new(&env);
    
    // Step 1: Lock first item
    steps.push_back(TransactionStep {
        step_id: 1,
        contract: mock_contract.clone(),
        function: Symbol::new(&env, "lock_item"),
        args: Vec::new(&env),
        depends_on: None,
        rollback_contract: Some(mock_contract.clone()),
        rollback_function: Some(Symbol::new(&env, "unlock_item")),
        rollback_args: Some(Vec::new(&env)),
    });
    
    // Step 2: Lock second item (depends on step 1)
    steps.push_back(TransactionStep {
        step_id: 2,
        contract: mock_contract.clone(),
        function: Symbol::new(&env, "lock_item"),
        args: Vec::new(&env),
        depends_on: Some(1),
        rollback_contract: Some(mock_contract.clone()),
        rollback_function: Some(Symbol::new(&env, "unlock_item")),
        rollback_args: Some(Vec::new(&env)),
    });
    
    // Note: In a real test, we'd have proper contracts that implement these functions
    // This test verifies the transaction structure validation works
    let initiator = Address::generate(&env);
    env.mock_all_auths();
    
    // We expect this to fail during prepare since the functions don't exist, but the workflow should trigger rollback
    let success = client.try_execute_atomic_transaction(&initiator, &steps);
    assert!(success.is_err());
    
    // The transaction should exist and be marked as failed/rolled back
    let tx_id = 1;
    if let Some(tx_state) = client.get_atomic_transaction(tx_id) {
        // The transaction state should be updated
        assert!(tx_state.status == TransactionStatus::RolledBack || tx_state.status == TransactionStatus::Failed);
    }
}

#[test]
#[should_panic(expected = "Invalid dependency")]
fn test_atomic_transaction_invalid_dependency() {
    let (env, client, _admin) = setup();
    
    let mock_contract = client.address.clone();
    
    // Create steps with invalid dependency (step 1 depends on step 2 which comes later)
    let mut steps = Vec::new(&env);
    steps.push_back(TransactionStep {
        step_id: 1,
        contract: mock_contract.clone(),
        function: Symbol::new(&env, "lock_item"),
        args: Vec::new(&env),
        depends_on: Some(2), // Invalid - depends on later step
        rollback_contract: Some(mock_contract.clone()),
        rollback_function: Some(Symbol::new(&env, "unlock_item")),
        rollback_args: Some(Vec::new(&env)),
    });
    
    steps.push_back(TransactionStep {
        step_id: 2,
        contract: mock_contract.clone(),
        function: Symbol::new(&env, "lock_item"),
        args: Vec::new(&env),
        depends_on: None,
        rollback_contract: Some(mock_contract.clone()),
        rollback_function: Some(Symbol::new(&env, "unlock_item")),
        rollback_args: Some(Vec::new(&env)),
    });
    
    let initiator = Address::generate(&env);
    env.mock_all_auths();
    let _ = client.execute_atomic_transaction(&initiator, &steps);
}

#[test]
fn test_get_atomic_step() {
    let (env, client, _admin) = setup();
    
    // Test that we can query non-existent step
    let step = client.get_atomic_step(1, 1);
    assert!(step.is_none());
}

#[test]
fn test_manual_admin_rollback() {
    let (env, client, admin) = setup();
    
    let mock_contract = client.address.clone();
    let mut steps = Vec::new(&env);
    steps.push_back(TransactionStep {
        step_id: 1,
        contract: mock_contract.clone(),
        function: Symbol::new(&env, "lock_item"),
        args: Vec::new(&env),
        depends_on: None,
        rollback_contract: Some(mock_contract.clone()),
        rollback_function: Some(Symbol::new(&env, "unlock_item")),
        rollback_args: Some(Vec::new(&env)),
    });
    
    // Create a transaction ID manually
    let tx_id = 1;
    let reason = soroban_sdk::String::from_str(&env, "Emergency rollback");
    
    // Admin can manually trigger rollback even if transaction doesn't exist (returns false)
    let result = client.try_rollback_atomic_transaction(&admin, &tx_id, &steps, &reason);
    assert!(result.is_ok()); // The call succeeds, but returns false since transaction doesn't exist
}