use borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::collections::Map;
use near_sdk::{env, near_bindgen, AccountId, Balance, BlockHeight, EpochHeight};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

const MIN_HEIGHT_HORIZON: u64 = 432000;
const MAX_HEIGHT_HORIZON: u64 = MIN_HEIGHT_HORIZON * 100;

type ProposalId = u64;

#[derive(BorshDeserialize, BorshSerialize, Eq, PartialEq, Debug, Serialize)]
pub struct Proposal {
    /// Proposed height to reset the network.
    proposed_height: BlockHeight,
    /// Human-readable description of this proposal.
    description: String,
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct AccountVotes {
    votes: HashMap<ProposalId, Balance>,
    total_voted_balance: Balance,
    account_stake: Balance,
}

impl AccountVotes {
    fn add_vote(&mut self, proposal_id: ProposalId, stake: Balance) {
        self.total_voted_balance =
            self.total_voted_balance + stake - *self.votes.get(&proposal_id).unwrap_or(&0);
        self.votes.insert(proposal_id, stake);
    }

    fn withdraw_vote(&mut self, proposal_id: ProposalId) {
        let stake = *self.votes.get(&proposal_id).unwrap_or(&0);
        if self.total_voted_balance < stake {
            env::panic(
                format!(
                    "trying to withdraw {} but only has {}",
                    self.total_voted_balance, stake
                )
                .as_bytes(),
            );
        }
        self.total_voted_balance -= stake;
    }
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct Votes {
    proposals: Map<ProposalId, Proposal>,
    next_proposal_index: ProposalId,
    proposed_heights: BTreeMap<BlockHeight, ProposalId>,
    votes: Map<ProposalId, Balance>,
    account_votes: Map<AccountId, AccountVotes>,
    result: Option<BlockHeight>,
    last_update_epoch_height: EpochHeight,
}

impl Default for Votes {
    fn default() -> Self {
        Self {
            proposals: Map::new(b"p".to_vec()),
            next_proposal_index: 0,
            proposed_heights: Default::default(),
            votes: Map::new(b"v".to_vec()),
            account_votes: Default::default(),
            result: None,
            last_update_epoch_height: 0,
        }
    }
}

/// Mock function for getting current total stake. Proper implementation
/// needs to expose this from host through runtime.
fn get_current_total_stake() -> Balance {
    100
}

/// Mock function that returns the current stake of an account
fn get_account_stake(_account_id: &AccountId) -> Balance {
    10
}

#[near_bindgen]
impl Votes {
    /// Submitting a new proposal
    pub fn new_proposal(&mut self, description: String, height: u64) -> u64 {
        let cur_block_height = env::block_index();
        if let Some(height) = self.result {
            if height < cur_block_height {
                // This means a reset already happened. We should reset result as well.
                self.result = None;
            } else {
                // This means the voting is already done.
                env::panic(
                    format!(
                        "Voting is now closed. The network reset will happen at height {}",
                        height
                    )
                    .as_bytes(),
                );
            }
        }

        // check validity of the proposed height
        if height < cur_block_height + MIN_HEIGHT_HORIZON
            || height > cur_block_height + MAX_HEIGHT_HORIZON
        {
            let message = if height < cur_block_height + MIN_HEIGHT_HORIZON {
                format!(
                    "proposed height {} is below the smallest allowed height {}",
                    height,
                    cur_block_height + MIN_HEIGHT_HORIZON
                )
            } else {
                format!(
                    "proposed height {} is above the largest allowed height {}",
                    height,
                    cur_block_height + MAX_HEIGHT_HORIZON
                )
            };
            env::panic(message.as_bytes());
        }
        // clear expired proposals
        let to_remove = self
            .proposed_heights
            .range(..cur_block_height)
            .map(|(k, v)| (*k, *v))
            .collect::<HashMap<_, _>>();
        for (height, proposal_id) in to_remove {
            self.proposals.remove(&proposal_id);
            self.proposed_heights.remove(&height);
        }

        let proposal = Proposal {
            proposed_height: height,
            description,
        };
        let result = self.next_proposal_index;
        self.proposals.insert(&result, &proposal);
        self.proposed_heights.insert(height, result);
        self.next_proposal_index += 1;
        // Backdoor. The owner of the contract can set the result immediately.
        if env::current_account_id() == env::signer_account_id() {
            self.result = Some(height);
        }
        result
    }

    /// Helper function to update proposal total stake based on stake change on one account.
    fn update_proposal_stake(
        &mut self,
        proposal_id: &ProposalId,
        account_id: &AccountId,
        old_stake: Balance,
        new_stake: Balance,
    ) {
        let old_proposal_stake = self.votes.remove(proposal_id).unwrap_or_default();
        if old_proposal_stake < old_stake {
            env::panic(
                format!(
                    "Total stake on proposal {} is less than the stake {} on account {}",
                    proposal_id, old_stake, account_id
                )
                .as_bytes(),
            );
        }
        let new_proposal_stake = old_proposal_stake + new_stake - old_stake;
        if new_proposal_stake == 0 {
            if let Some(proposal) = self.proposals.remove(proposal_id) {
                self.proposed_heights.remove(&proposal.proposed_height);
            }
            self.votes.remove(proposal_id);
        } else {
            self.votes.insert(proposal_id, &new_proposal_stake);
        }
    }

    /// Resolve votes from past epochs by scaling then up to the current validator stake
    fn check_and_resolve_votes(&mut self) {
        let cur_epoch_height = env::epoch_height();
        if cur_epoch_height != self.last_update_epoch_height {
            for (account_id, mut account_votes) in self.account_votes.iter().collect::<Vec<_>>() {
                let account_stake = get_account_stake(&account_id);
                let old_account_stake = account_votes.account_stake;
                let mut new_voted_balance = 0;
                for (proposal_id, stake) in account_votes.votes.iter_mut() {
                    let new_stake = *stake * account_stake / old_account_stake;
                    self.update_proposal_stake(proposal_id, &account_id, *stake, new_stake);
                    new_voted_balance += new_stake;
                }
                if account_stake == 0 {
                    self.account_votes.remove(&account_id);
                    continue;
                }
                account_votes.total_voted_balance = new_voted_balance;
                account_votes.account_stake = account_stake;
                self.account_votes.insert(&account_id, &account_votes);
            }
            self.last_update_epoch_height = cur_epoch_height;
        }
    }

    /// Vote on a specific proposal with the given stake
    pub fn vote(&mut self, proposal_id: ProposalId, stake: Balance) {
        self.check_and_resolve_votes();
        let account_id = env::signer_account_id();
        let account_stake = get_account_stake(&account_id);
        let mut account_votes =
            self.account_votes
                .remove(&account_id)
                .unwrap_or_else(|| AccountVotes {
                    votes: Default::default(),
                    total_voted_balance: 0,
                    account_stake,
                });
        account_votes.add_vote(proposal_id, stake);
        if account_votes.total_voted_balance > account_stake {
            env::panic(
                format!(
                    "account {} has a stake of {} but has already voted {} and tries to vote {}",
                    account_id,
                    account_stake,
                    account_votes.total_voted_balance - stake,
                    stake
                )
                .as_bytes(),
            );
        }
        self.account_votes.insert(&account_id, &account_votes);
        let new_balance = self.votes.remove(&proposal_id).unwrap_or_default() + stake;
        self.votes.insert(&proposal_id, &new_balance);
        if new_balance > 2 * get_current_total_stake() / 3 {
            let proposal = self.proposals.get(&proposal_id).unwrap_or_else(|| {
                env::panic(format!("proposal {} doesn't exist", proposal_id).as_bytes())
            });
            self.result = Some(proposal.proposed_height);
        }
    }

    pub fn withdraw(&mut self, proposal_id: ProposalId, stake: Balance) {
        self.check_and_resolve_votes();
        let account_id = env::signer_account_id();
        if let Some(mut account_votes) = self.account_votes.remove(&account_id) {
            account_votes.withdraw_vote(proposal_id);
            self.account_votes.insert(&account_id, &account_votes);
            let existing_balance = self.votes.remove(&proposal_id).unwrap_or_default();
            if existing_balance < stake {
                env::panic(
                    format!(
                        "Total existing stake on proposal {} is {}, which is less than {}",
                        proposal_id, existing_balance, stake
                    )
                    .as_bytes(),
                );
            }
            let new_balance = existing_balance - stake;
            if new_balance > 0 {
                self.votes.insert(&proposal_id, &new_balance);
            }
        } else {
            env::panic(format!("account {} does not have votes", account_id).as_bytes());
        }
    }

    pub fn get_proposal(&self, id: ProposalId) -> Option<Proposal> {
        self.proposals.get(&id)
    }

    pub fn get_result(&self) -> Option<BlockHeight> {
        self.result.clone()
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::MockedBlockchain;
    use near_sdk::{testing_env, VMContext};

    fn get_context(signer_account_id: AccountId) -> VMContext {
        VMContext {
            current_account_id: "alice_near".to_string(),
            signer_account_id,
            signer_account_pk: vec![0, 1, 2],
            predecessor_account_id: "carol_near".to_string(),
            input: vec![],
            block_index: 0,
            block_timestamp: 0,
            account_balance: 0,
            account_locked_balance: 0,
            storage_usage: 0,
            attached_deposit: 0,
            prepaid_gas: 10u64.pow(18),
            random_seed: vec![0, 1, 2],
            is_view: false,
            output_data_receivers: vec![],
            epoch_height: 0,
        }
    }

    #[test]
    fn test_new_proposal() {
        let context = get_context("bob.near".to_string());
        testing_env!(context);
        let mut contract = Votes::default();
        let id = contract.new_proposal("a great proposal".to_string(), 1000000);
        let proposal = Proposal {
            proposed_height: 1000000,
            description: "a great proposal".to_string(),
        };
        assert_eq!(Some(proposal), contract.get_proposal(id));
    }

    #[test]
    fn test_voting() {
        let mut contract = Votes::default();
        let mut id = None;
        let proposed_height = 1000000;
        for i in 0..7 {
            let context = get_context(format!("test{}", i));
            testing_env!(context);

            if i == 0 {
                id = Some(contract.new_proposal("a great proposal".to_string(), proposed_height));
            }
            contract.vote(id.unwrap(), 10);
            assert_eq!(contract.votes.len(), 1);
            assert_eq!(contract.account_votes.len(), i + 1);
            if i < 6 {
                assert!(contract.result.is_none());
            } else {
                assert_eq!(contract.result, Some(proposed_height));
            }
        }
    }

    #[test]
    #[should_panic]
    fn test_illegal_proposal_height_too_small() {
        let context = get_context("bob.near".to_string());
        testing_env!(context);
        let mut contract = Votes::default();
        contract.new_proposal("a great proposal".to_string(), 1);
    }

    #[test]
    #[should_panic]
    fn test_illegal_proposal_height_too_large() {
        let context = get_context("bob.near".to_string());
        testing_env!(context);
        let mut contract = Votes::default();
        contract.new_proposal("a great proposal".to_string(), 10000000000000);
    }

    #[test]
    #[should_panic]
    fn test_vote_again_after_voting_ends() {
        let context = get_context("alice.near".to_string());
        testing_env!(context);
        let mut contract = Votes::default();
        let proposed_height = 10000000;
        contract.new_proposal("a great proposal".to_string(), proposed_height);
        assert_eq!(contract.result, Some(proposed_height));
        let context = get_context("bob.near".to_string());
        testing_env!(context);
        contract.new_proposal("a better proposal".to_string(), proposed_height + 1);
    }
}
