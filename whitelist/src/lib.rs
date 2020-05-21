use borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::collections::Set;
use near_sdk::{env, near_bindgen, AccountId};

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct WhitelistContract {
    /// The account ID of the NEAR Foundation. It allows to whitelist new staking pool accounts.
    /// It also allows to whitelist new Staking Pool Factories, which can whitelist staking pools.
    pub foundation_account_id: AccountId,
    /// The whitelisted account IDs of approved staking pool contracts.
    pub whitelist: Set<AccountId>,
}

impl Default for WhitelistContract {
    fn default() -> Self {
        env::panic(b"The contract should be initialized before usage")
    }
}

#[near_bindgen]
impl WhitelistContract {
    #[init]
    pub fn new(foundation_account_id: AccountId) -> Self {
        assert!(!env::state_exists(), "Already initialized");
        assert!(
            env::is_valid_account_id(foundation_account_id.as_bytes()),
            "The NEAR Foundation account ID is invalid"
        );
        Self {
            foundation_account_id,
            whitelist: Set::new(b"w".to_vec()),
        }
    }

    /// Adds the given staking pool account ID to the whitelist.
    /// Returns `true` if the staking pool was not in the whitelist before, `false` otherwise.
    pub fn add_staking_pool(&mut self, staking_pool_account_id: AccountId) -> bool {
        self.assert_called_by_foundation();
        assert!(
            env::is_valid_account_id(staking_pool_account_id.as_bytes()),
            "The given account ID is invalid"
        );
        self.whitelist.insert(&staking_pool_account_id)
    }

    /// Removes the given staking pool account ID from the whitelist.
    /// Returns `true` if the staking pool was present in the whitelist before, `false` otherwise.
    pub fn remove_staking_pool(&mut self, staking_pool_account_id: AccountId) -> bool {
        self.assert_called_by_foundation();
        assert!(
            env::is_valid_account_id(staking_pool_account_id.as_bytes()),
            "The given account ID is invalid"
        );
        self.whitelist.remove(&staking_pool_account_id)
    }

    /// Returns `true` if the given staking pool account ID is whitelisted.
    pub fn is_whitelisted(&self, staking_pool_account_id: AccountId) -> bool {
        assert!(
            env::is_valid_account_id(staking_pool_account_id.as_bytes()),
            "The given account ID is invalid"
        );
        self.whitelist.contains(&staking_pool_account_id)
    }

    /// Internal method to verify the predecessor was the NEAR Foundation account ID.
    fn assert_called_by_foundation(&self) {
        assert_eq!(
            &env::predecessor_account_id(),
            &self.foundation_account_id,
            "Can only be called by NEAR Foundation"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::{testing_env, MockedBlockchain};

    mod test_utils;
    use test_utils::*;

    #[test]
    fn test_whitelist() {
        let mut context = VMContextBuilder::new()
            .current_account_id(account_whitelist())
            .predecessor_account_id(account_near())
            .finish();
        testing_env!(context.clone());

        let mut contract = WhitelistContract::new(account_near());

        // Check initial whitelist
        context.is_view = true;
        testing_env!(context.clone());
        assert!(!contract.is_whitelisted(account_pool()));

        // Adding to whitelist by foundation
        context.is_view = false;
        testing_env!(context.clone());
        assert!(contract.add_staking_pool(account_pool()));

        // Checking it's whitelisted now
        context.is_view = true;
        testing_env!(context.clone());
        assert!(contract.is_whitelisted(account_pool()));

        // Adding again. Should return false
        context.is_view = false;
        testing_env!(context.clone());
        assert!(!contract.add_staking_pool(account_pool()));

        // Checking the pool is still whitelisted
        context.is_view = true;
        testing_env!(context.clone());
        assert!(contract.is_whitelisted(account_pool()));

        // Removing from the whitelist.
        context.is_view = false;
        testing_env!(context.clone());
        assert!(contract.remove_staking_pool(account_pool()));

        // Checking the pool is not whitelisted anymore
        context.is_view = true;
        testing_env!(context.clone());
        assert!(!contract.is_whitelisted(account_pool()));

        // Removing again from the whitelist, should return false.
        context.is_view = false;
        testing_env!(context.clone());
        assert!(!contract.remove_staking_pool(account_pool()));

        // Checking the pool is still not whitelisted
        context.is_view = true;
        testing_env!(context.clone());
        assert!(!contract.is_whitelisted(account_pool()));

        // Adding again after it was removed. Should return true
        context.is_view = false;
        testing_env!(context.clone());
        assert!(contract.add_staking_pool(account_pool()));

        // Checking the pool is now whitelisted again
        context.is_view = true;
        testing_env!(context.clone());
        assert!(contract.is_whitelisted(account_pool()));
    }
}
