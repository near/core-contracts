use near_sdk::AccountId;
use near_sdk::Balance;

pub fn staking() -> AccountId {
    "staking".parse().unwrap()
}

pub fn alice() -> AccountId {
    "alice".parse().unwrap()
}
pub fn bob() -> AccountId {
    "bob".parse().unwrap()
}
pub fn owner() -> AccountId {
    "owner".parse().unwrap()
}

pub fn ntoy(near_amount: Balance) -> Balance {
    near_amount * 10u128.pow(24)
}

/// Rounds to nearest
pub fn yton(yocto_amount: Balance) -> Balance {
    (yocto_amount + (5 * 10u128.pow(23))) / 10u128.pow(24)
}

#[macro_export]
macro_rules! assert_eq_in_near {
    ($a:expr, $b:expr) => {
        assert_eq!(yton($a), yton($b))
    };
    ($a:expr, $b:expr, $c:expr) => {
        assert_eq!(yton($a), yton($b), $c)
    };
}
