use borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::json_types::U64;
use near_sdk::{near_bindgen};

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, Default)]
pub struct FakeVotingContract {}

#[near_bindgen]
impl FakeVotingContract {
    pub fn get_result(&self) -> Option<U64> {
        Some(1535760000000000000u64.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::{testing_env, MockedBlockchain};

    mod test_utils;
    use test_utils::*;

    #[test]
    fn test_get_result() {
        let mut context = VMContextBuilder::new()
            .current_account_id(account_whitelist())
            .predecessor_account_id(account_near())
            .finish();
        testing_env!(context.clone());

        let contract: FakeVotingContract = Default::default();

        // Check initial whitelist
        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(
            contract.get_result().unwrap().0,
            1535760000000000000u64,
        );
    }
}
