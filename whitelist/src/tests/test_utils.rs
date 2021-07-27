use near_sdk::AccountId;

pub fn account_near() -> AccountId {
    "near".parse().unwrap()
}
pub fn account_whitelist() -> AccountId {
    "whitelist".parse().unwrap()
}
pub fn account_pool() -> AccountId {
    "pool".parse().unwrap()
}
pub fn account_factory() -> AccountId {
    "factory".parse().unwrap()
}
