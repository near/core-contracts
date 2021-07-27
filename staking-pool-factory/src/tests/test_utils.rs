use near_sdk::AccountId;
use near_sdk::Balance;

pub fn account_near() -> AccountId {
    "near".parse().unwrap()
}
pub fn account_whitelist() -> AccountId {
    "whitelist".parse().unwrap()
}
pub fn staking_pool_id() -> String {
    "pool".to_string()
}
pub fn account_pool() -> AccountId {
    "pool.factory".parse().unwrap()
}
pub fn account_factory() -> AccountId {
    "factory".parse().unwrap()
}
pub fn account_tokens_owner() -> AccountId {
    "tokens-owner".parse().unwrap()
}
pub fn account_pool_owner() -> AccountId {
    "pool-owner".parse().unwrap()
}

pub fn ntoy(near_amount: Balance) -> Balance {
    near_amount * 10u128.pow(24)
}
