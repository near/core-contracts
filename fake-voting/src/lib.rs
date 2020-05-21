use borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::json_types::U64;
use near_sdk::{near_bindgen, BlockHeight};
use serde::Serialize;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, Default)]
pub struct FakeVotingContract {}

#[derive(Serialize)]
pub struct PollResult {
    /// The proposal ID that was voted in.
    pub proposal_id: u64,
    /// The timestamp when the proposal was voted in.
    pub timestamp: U64,
    /// The block height when the proposal was voted in.
    pub block_height: BlockHeight,
}

#[near_bindgen]
impl FakeVotingContract {
    pub fn get_result(&self) -> Option<PollResult> {
        Some(PollResult {
            proposal_id: 0,
            timestamp: 1535760000000000000u64.into(),
            block_height: 0,
        })
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
            contract.get_result().unwrap().timestamp.0,
            1535760000000000000u64,
        );
    }
}
