use soroban_sdk::{contracttype, Address, Symbol};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OracleData {
    pub metric_id: Symbol,
    pub value: u128,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PricingRule {
    pub base_price: u128,
    pub metric_id: Symbol,
    pub scale_factor_bps: u32,
    pub inverse: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MarketplaceCircuitBreaker {
    Active,
    Terminated,
}
