use borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::collections::Map;
use near_sdk::{env, near_bindgen, AccountId, Balance, EpochHeight};
use uint::construct_uint;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

type Timestamp = u64;

construct_uint! {
    /// 256-bit unsigned integer.
    // TODO: Revert back to 4 once wasm/wasmer bug is fixed.
    // See https://github.com/wasmerio/wasmer/issues/1429
    pub struct U256(8);
}

/// Voting contract for unlocking transfers. Once the majority of the stake holders agree to
/// unlock transfer, the time will be recorded and the voting ends.
#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct VotingContract {
    /// How much each validator votes
    votes: Map<AccountId, Balance>,
    /// Map of account to their current stake
    account_stake: Map<AccountId, Balance>,
    /// Total voted balance so far.
    total_voted_stake: Balance,
    /// When the voting ended. `None` means the poll is still open.
    result: Option<Timestamp>,
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
        VotingContract {
            votes: Map::new(b"a".to_vec()),
            account_stake: Map::new(b"s".to_vec()),
            total_voted_stake: 0,
            result: None,
            last_epoch_height: 0,
        }
    }

    fn resolve_votes(&mut self) {
        let cur_epoch_height = env::epoch_height();
        if cur_epoch_height != self.last_epoch_height {
            for account_id in self.account_stake.keys().into_iter().collect::<Vec<_>>() {
                let old_account_stake = self.account_stake.remove(&account_id).unwrap();
                let account_current_stake = env::validator_stake(&account_id);
                let account_voted_stake = self.votes.remove(&account_id).unwrap();
                if account_current_stake > 0 {
                    let new_account_voted_stake = (U256::from(account_voted_stake)
                        * U256::from(account_current_stake)
                        / U256::from(old_account_stake))
                    .as_u128();
                    self.total_voted_stake =
                        self.total_voted_stake + new_account_voted_stake - account_voted_stake;
                    self.votes.insert(&account_id, &new_account_voted_stake);
                    self.account_stake
                        .insert(&account_id, &account_current_stake);
                }
            }
            self.check_result();
            self.last_epoch_height = cur_epoch_height;
        }
    }

    fn check_result(&mut self) {
        assert!(
            self.result.is_none(),
            "check result is called after result is already set"
        );
        let total_stake = env::validator_total_stake();
        if self.total_voted_stake > 2 * total_stake / 3 {
            self.result = Some(env::block_timestamp());
        }
    }

    /// Vote on a specific proposal with the given stake
    pub fn vote(&mut self, stake: Balance) {
        if self.result.is_some() {
            env::panic("voting has already ended".as_bytes());
        }
        self.resolve_votes();
        if self.result.is_some() {
            return;
        }
        let account_id = env::predecessor_account_id();
        let account_stake = env::validator_stake(&account_id);
        assert!(
            account_stake > 0,
            "account {} is not a validator",
            account_id
        );
        if stake > account_stake {
            env::panic(
                format!(
                    "account {} has a stake of {} but tries to vote {}",
                    account_id, account_stake, stake
                )
                .as_bytes(),
            );
        }
        let voted_stake = self.votes.remove(&account_id).unwrap_or_default();
        assert!(
            voted_stake <= self.total_voted_stake,
            "voted stake {} is more than total voted stake {}",
            voted_stake,
            self.total_voted_stake
        );
        if stake == 0 {
            self.account_stake.remove(&account_id);
            return;
        }
        self.votes.insert(&account_id, &stake);
        self.account_stake.insert(&account_id, &account_stake);
        self.total_voted_stake = self.total_voted_stake + stake - voted_stake;
        self.check_result();
    }

    pub fn get_result(&self) -> Option<Timestamp> {
        self.result.clone()
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
        contract.vote(100);
    }

    #[test]
    #[should_panic(expected = "voting has already ended")]
    fn test_vote_again_after_voting_ends() {
        let context = get_context("alice.near".to_string());
        let validators = HashMap::from_iter(vec![("alice.near".to_string(), 100)].into_iter());
        testing_env!(context, Default::default(), Default::default(), validators);
        let mut contract = VotingContract::new();
        contract.vote(100);
        assert!(contract.result.is_some());
        contract.vote(1);
    }

    #[test]
    fn test_voting_simple() {
        let mut contract = VotingContract::new();
        let validators = (0..10)
            .map(|i| (format!("test{}", i), 10))
            .collect::<HashMap<_, _>>();

        for i in 0..7 {
            let context = get_context(format!("test{}", i));
            testing_env!(
                context,
                Default::default(),
                Default::default(),
                validators.clone()
            );
            contract.vote(10);
            assert_eq!(contract.votes.len(), i + 1);
            assert_eq!(contract.account_stake.len(), i + 1);
            if i < 6 {
                assert!(contract.result.is_none());
            } else {
                assert!(contract.result.is_some());
            }
        }
    }

    #[test]
    fn test_voting_with_epoch_change() {
        let mut contract = VotingContract::new();
        let validators = (0..10)
            .map(|i| (format!("test{}", i), 10))
            .collect::<HashMap<_, _>>();
        for i in 0..7 {
            let context = get_context_with_epoch_height(format!("test{}", i), i);
            testing_env!(
                context,
                Default::default(),
                Default::default(),
                validators.clone()
            );
            contract.vote(10);
            assert_eq!(contract.votes.len(), i + 1);
            assert_eq!(contract.account_stake.len(), i + 1);
            if i < 6 {
                assert!(contract.result.is_none());
            } else {
                assert!(contract.result.is_some());
            }
        }
    }

    #[test]
    fn test_validator_stake_change() {
        let mut contract = VotingContract::new();
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
        contract.vote(40);
        validators.insert("test1".to_string(), 50);
        let context = get_context_with_epoch_height("test2".to_string(), 2);
        testing_env!(
            context,
            Default::default(),
            Default::default(),
            validators.clone()
        );
        contract.vote(5);
        assert!(contract.result.is_some());
    }

    #[test]
    fn test_withdraw_votes() {
        let mut contract = VotingContract::new();
        let validators =
            HashMap::from_iter(vec![("test1".to_string(), 10), ("test2".to_string(), 10)]);
        let context = get_context_with_epoch_height("test1".to_string(), 1);
        testing_env!(
            context,
            Default::default(),
            Default::default(),
            validators.clone()
        );
        contract.vote(5);
        assert_eq!(contract.account_stake.len(), 1);
        assert_eq!(contract.votes.len(), 1);
        let context = get_context_with_epoch_height("test1".to_string(), 2);
        testing_env!(
            context,
            Default::default(),
            Default::default(),
            validators.clone()
        );
        contract.vote(0);
        assert!(contract.account_stake.is_empty());
        assert!(contract.votes.is_empty());
    }
}
