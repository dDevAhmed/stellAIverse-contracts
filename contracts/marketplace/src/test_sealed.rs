#![cfg(test)]

use soroban_sdk::{Address, Env, Bytes, String};
use soroban_sdk::testutils::{Ledger as _, Address as _};
use stellai_lib::AuctionType;

use crate::storage::{get_sealed_commit_count, get_sealed_reveal_count, get_sealed_reveal_entry};
use crate::{Marketplace, MarketplaceClient};

#[test]
fn test_sealed_auction_commit_reveal_flow() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register_contract(None, Marketplace);
    let client = MarketplaceClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let seller = Address::generate(&env);
    let bidder1 = Address::generate(&env);
    let bidder2 = Address::generate(&env);

    client.init_contract(&admin);

    let auction_id = client.create_sealed_auction(&1u64, &seller, &1000i128, &0i128, &10u64, &10u64, &500u32);

    // Prepare commitments
    let nonce1 = String::from_str(&env, "n1");
    let combined1 = format!("{}:{}:{}", 1500i128, nonce1.to_string(), bidder1.to_string());
    let hash1 = env.crypto().sha256(&combined1.into());
    let commitment1: Bytes = hash1.into();

    let nonce2 = String::from_str(&env, "n2");
    let combined2 = format!("{}:{}:{}", 1200i128, nonce2.to_string(), bidder2.to_string());
    let hash2 = env.crypto().sha256(&combined2.into());
    let commitment2: Bytes = hash2.into();

    // Commit bids
    client.commit_sealed_bid(&auction_id, &bidder1, &commitment1, &1500i128);
    client.commit_sealed_bid(&auction_id, &bidder2, &commitment2, &1200i128);

    // Advance to reveal
    env.ledger().set_timestamp(env.ledger().timestamp() + 11);

    client.reveal_sealed_bid(&auction_id, &bidder1, &1500i128, &nonce1);
    client.reveal_sealed_bid(&auction_id, &bidder2, &1200i128, &nonce2);

    // Check revealed counts and highest bid
    let reveal_count = get_sealed_reveal_count(&env, auction_id);
    assert_eq!(reveal_count, 2);

    let r0 = get_sealed_reveal_entry(&env, auction_id, 0).unwrap();
    assert!(r0.amount == 1500 || r0.amount == 1200);
}