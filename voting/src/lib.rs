use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::json_types::{U128, U64};
use near_sdk::{env, near_bindgen, AccountId, Balance, EpochHeight};
use std::collections::HashMap;

#[global_allocator]
static ALLOC: near_sdk::wee_alloc::WeeAlloc = near_sdk::wee_alloc::WeeAlloc::INIT;

type WrappedTimestamp = U64;

/// Voting contract for unlocking transfers. Once the majority of the stake holders agree to
/// unlock transfer, the time will be recorded and the voting ends.
#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct VotingContract {
    /// How much each validator votes
    votes: HashMap<AccountId, Balance>,
    /// Total voted balance so far.
    total_voted_stake: Balance,
    /// When the voting ended. `None` means the poll is still open.
    result: Option<WrappedTimestamp>,
    /// Epoch height when the contract is touched last time.
    last_epoch_height: EpochHeight,
}

impl Default for VotingContract {
    fn default() -> Self {
        env::panic(b"Voting contract should be initialized before usage")
    }
}

#[near_bindgen]
impl VotingContract {
    #[init]
    pub fn new() -> Self {
        assert!(!env::state_exists(), "The contract is already initialized");
        VotingContract {
            votes: HashMap::new(),
            total_voted_stake: 0,
            result: None,
            last_epoch_height: 0,
        }
    }

    /// Ping to update the votes according to current stake of validators.
    pub fn ping(&mut self) {
        assert!(self.result.is_none(), "Voting has already ended");
        let cur_epoch_height = env::epoch_height();
        if cur_epoch_height != self.last_epoch_height {
            let votes = std::mem::take(&mut self.votes);
            self.total_voted_stake = 0;
            for (account_id, _) in votes {
                let account_current_stake = env::validator_stake(&account_id);
                self.total_voted_stake += account_current_stake;
                if account_current_stake > 0 {
                    self.votes.insert(account_id, account_current_stake);
                }
            }
            self.check_result();
            self.last_epoch_height = cur_epoch_height;
        }
    }

    /// Check whether the voting has ended.
    fn check_result(&mut self) {
        assert!(
            self.result.is_none(),
            "check result is called after result is already set"
        );
        let total_stake = env::validator_total_stake();
        if self.total_voted_stake > 2 * total_stake / 3 {
            self.result = Some(U64::from(env::block_timestamp()));
        }
    }

    /// Method for validators to vote or withdraw the vote.
    /// Votes for if `is_vote` is true, or withdraws the vote if `is_vote` is false.
    pub fn vote(&mut self, is_vote: bool) {
        self.ping();
        if self.result.is_some() {
            return;
        }
        let account_id = env::predecessor_account_id();
        let account_stake = if is_vote {
            let stake = env::validator_stake(&account_id);
            assert!(stake > 0, "{} is not a validator", account_id);
            stake
        } else {
            0
        };
        let voted_stake = self.votes.remove(&account_id).unwrap_or_default();
        assert!(
            voted_stake <= self.total_voted_stake,
            "invariant: voted stake {} is more than total voted stake {}",
            voted_stake,
            self.total_voted_stake
        );
        self.total_voted_stake = self.total_voted_stake + account_stake - voted_stake;
        if account_stake > 0 {
            self.votes.insert(account_id, account_stake);
            self.check_result();
        }
    }

    /// Get the timestamp of when the voting finishes. `None` means the voting hasn't ended yet.
    pub fn get_result(&self) -> Option<WrappedTimestamp> {
        self.result.clone()
    }

    /// Returns current a pair of `total_voted_stake` and the total stake.
    /// Note: as a view method, it doesn't recompute the active stake. May need to call `ping` to
    /// update the active stake.
    pub fn get_total_voted_stake(&self) -> (U128, U128) {
        (
            self.total_voted_stake.into(),
            env::validator_total_stake().into(),
        )
    }

    /// Returns all active votes.
    /// Note: as a view method, it doesn't recompute the active stake. May need to call `ping` to
    /// update the active stake.
    pub fn get_votes(&self) -> HashMap<AccountId, U128> {
        self.votes
            .iter()
            .map(|(account_id, stake)| (account_id.clone(), (*stake).into()))
            .collect()
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::MockedBlockchain;
    use near_sdk::{testing_env, VMContext};
    use std::collections::HashMap;
    use std::iter::FromIterator;

    fn get_context(predecessor_account_id: AccountId) -> VMContext {
        get_context_with_epoch_height(predecessor_account_id, 0)
    }

    fn get_context_with_epoch_height(
        predecessor_account_id: AccountId,
        epoch_height: EpochHeight,
    ) -> VMContext {
        VMContext {
            current_account_id: "alice_near".to_string(),
            signer_account_id: "bob_near".to_string(),
            signer_account_pk: vec![0, 1, 2],
            predecessor_account_id,
            input: vec![],
            block_index: 0,
            block_timestamp: 0,
            account_balance: 0,
            account_locked_balance: 0,
            storage_usage: 1000,
            attached_deposit: 0,
            prepaid_gas: 2 * 10u64.pow(14),
            random_seed: vec![0, 1, 2],
            is_view: false,
            output_data_receivers: vec![],
            epoch_height,
        }
    }

    #[test]
    #[should_panic(expected = "is not a validator")]
    fn test_nonvalidator_cannot_vote() {
        let context = get_context("bob.near".to_string());
        let validators = HashMap::from_iter(
            vec![
                ("alice_near".to_string(), 100),
                ("bob_near".to_string(), 100),
            ]
            .into_iter(),
        );
        testing_env!(context, Default::default(), Default::default(), validators);
        let mut contract = VotingContract::new();
        contract.vote(true);
    }

    #[test]
    #[should_panic(expected = "Voting has already ended")]
    fn test_vote_again_after_voting_ends() {
        let context = get_context("alice.near".to_string());
        let validators = HashMap::from_iter(vec![("alice.near".to_string(), 100)].into_iter());
        testing_env!(context, Default::default(), Default::default(), validators);
        let mut contract = VotingContract::new();
        contract.vote(true);
        assert!(contract.result.is_some());
        contract.vote(true);
    }

    #[test]
    fn test_voting_simple() {
        let context = get_context("test0".to_string());
        let validators = (0..10)
            .map(|i| (format!("test{}", i), 10))
            .collect::<HashMap<_, _>>();
        testing_env!(
            context,
            Default::default(),
            Default::default(),
            validators.clone()
        );
        let mut contract = VotingContract::new();

        for i in 0..7 {
            let mut context = get_context(format!("test{}", i));
            testing_env!(
                context.clone(),
                Default::default(),
                Default::default(),
                validators.clone()
            );
            contract.vote(true);
            context.is_view = true;
            testing_env!(
                context,
                Default::default(),
                Default::default(),
                validators.clone()
            );
            assert_eq!(
                contract.get_total_voted_stake(),
                (U128::from(10 * (i + 1)), U128::from(100))
            );
            assert_eq!(
                contract.get_votes(),
                (0..=i)
                    .map(|i| (format!("test{}", i), U128::from(10)))
                    .collect::<HashMap<_, _>>()
            );
            assert_eq!(contract.votes.len() as u128, i + 1);
            if i < 6 {
                assert!(contract.result.is_none());
            } else {
                assert!(contract.result.is_some());
            }
        }
    }

    #[test]
    fn test_voting_with_epoch_change() {
        let validators = (0..10)
            .map(|i| (format!("test{}", i), 10))
            .collect::<HashMap<_, _>>();
        let context = get_context("test0".to_string());
        testing_env!(
            context,
            Default::default(),
            Default::default(),
            validators.clone()
        );
        let mut contract = VotingContract::new();

        for i in 0..7 {
            let context = get_context_with_epoch_height(format!("test{}", i), i);
            testing_env!(
                context,
                Default::default(),
                Default::default(),
                validators.clone()
            );
            contract.vote(true);
            assert_eq!(contract.votes.len() as u64, i + 1);
            if i < 6 {
                assert!(contract.result.is_none());
            } else {
                assert!(contract.result.is_some());
            }
        }
    }

    #[test]
    fn test_validator_stake_change() {
        let mut validators = HashMap::from_iter(vec![
            ("test1".to_string(), 40),
            ("test2".to_string(), 10),
            ("test3".to_string(), 10),
        ]);
        let context = get_context_with_epoch_height("test1".to_string(), 1);
        testing_env!(
            context,
            Default::default(),
            Default::default(),
            validators.clone()
        );

        let mut contract = VotingContract::new();
        contract.vote(true);
        validators.insert("test1".to_string(), 50);
        let context = get_context_with_epoch_height("test2".to_string(), 2);
        testing_env!(
            context,
            Default::default(),
            Default::default(),
            validators.clone()
        );
        contract.ping();
        assert!(contract.result.is_some());
    }

    #[test]
    fn test_withdraw_votes() {
        let validators =
            HashMap::from_iter(vec![("test1".to_string(), 10), ("test2".to_string(), 10)]);
        let context = get_context_with_epoch_height("test1".to_string(), 1);
        testing_env!(
            context,
            Default::default(),
            Default::default(),
            validators.clone()
        );
        let mut contract = VotingContract::new();
        contract.vote(true);
        assert_eq!(contract.votes.len(), 1);
        let context = get_context_with_epoch_height("test1".to_string(), 2);
        testing_env!(
            context,
            Default::default(),
            Default::default(),
            validators.clone()
        );
        contract.vote(false);
        assert!(contract.votes.is_empty());
    }

    #[test]
    fn test_validator_kick_out() {
        let mut validators = HashMap::from_iter(vec![
            ("test1".to_string(), 40),
            ("test2".to_string(), 10),
            ("test3".to_string(), 10),
        ]);
        let context = get_context_with_epoch_height("test1".to_string(), 1);
        testing_env!(
            context,
            Default::default(),
            Default::default(),
            validators.clone()
        );

        let mut contract = VotingContract::new();
        contract.vote(true);
        assert_eq!((contract.get_total_voted_stake().0).0, 40);
        validators.remove(&"test1".to_string());
        let context = get_context_with_epoch_height("test2".to_string(), 2);
        testing_env!(
            context,
            Default::default(),
            Default::default(),
            validators.clone()
        );
        contract.ping();
        assert_eq!((contract.get_total_voted_stake().0).0, 0);
    }
}
