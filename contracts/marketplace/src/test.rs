#![cfg(test)]
use soroban_sdk::{Env, Symbol, BytesN};
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::Address;

use crate::types::{PricingRule, OracleData};
use crate::{MarketplaceContract, MarketplaceContractClient};

#[test]
fn test_dynamic_pricing_calculation_upward() {
    let env = Env::default();
    let contract_id = env.register(MarketplaceContract, ());
    let client = MarketplaceContractClient::new(&env, &contract_id);

    let rule = PricingRule {
        base_price: 1000,
        metric_id: Symbol::new(&env, "PERF_INDEX"),
        scale_factor_bps: 2000, 
        inverse: false,
    };

    let dynamic_price = client.calculate_dynamic_price(&rule, &500);
    assert_eq!(dynamic_price, 1100);
}

#[test]
#[should_panic(expected = "Oracle data attestation has expired")]
fn test_stale_timestamp_rejection() {
    let env = Env::default();
    
    // Enable authentication mocking so require_auth_for_args succeeds automatically
    env.mock_all_auths();

    let contract_id = env.register(MarketplaceContract, ());
    let client = MarketplaceContractClient::new(&env, &contract_id);

    env.ledger().set_timestamp(10_000); 

    let stale_data = OracleData {
        metric_id: Symbol::new(&env, "VALUATION"),
        value: 250,
        timestamp: 1000, 
    };

    let authorized_oracle = Address::generate(&env);
    
    env.as_contract(&contract_id, || {
        env.storage().instance().set(&Symbol::new(&env, "oracle"), &authorized_oracle);
    });
    
    let mock_sig = BytesN::from_array(&env, &[0u8; 64]);
    client.verify_and_get_oracle_value(&stale_data, &mock_sig);
}
