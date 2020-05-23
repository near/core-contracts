use borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::collections::Map;
use near_sdk::json_types::{U128, U64};
use near_sdk::{env, near_bindgen, AccountId, Balance, BlockHeight, EpochHeight};
use serde::{Deserialize, Serialize};

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

const MIN_HEIGHT_HORIZON: u64 = 432000;
const MAX_HEIGHT_HORIZON: u64 = MIN_HEIGHT_HORIZON * 100;

type ProposalId = U64;

#[derive(BorshDeserialize, BorshSerialize, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct Fraction {
    pub numerator: u32,
    pub denominator: u32,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize)]
pub struct Proposal {
    /// Human readable description of the proposal.
    description: String,
    /// Serialized metadata of the proposal.
    metadata: String,
    /// When this proposal expires.
    expiration_height: BlockHeight,
    /// Current votes on this proposal.
    #[serde(skip)]
    votes: Map<AccountId, Balance>,
    /// participating accounts and their stake
    #[serde(skip)]
    account_stake: Map<AccountId, Balance>,
    /// Sum of voted stake.
    total_voted_stake: Balance,
    /// Height of the epoch when this proposal was last updated.
    last_epoch_height: EpochHeight,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct VotingContract {
    /// Human readable description of the poll.
    description: String,
    /// All proposals for this poll.
    proposals: Map<ProposalId, Proposal>,
    /// Accounts that have participated in this poll and the corresponding stake voted.
    accounts: Map<AccountId, Balance>,
    /// Map of account to their current stake
    account_stake: Map<AccountId, Balance>,
    /// Next proposal id.
    next_proposal_id: ProposalId,
    /// Threshold for closing the poll, i.e, if the ratio of stake on a certain proposal over total stake reaches
    /// threshold, the poll is closed.
    threshold: Fraction,
    /// Fee needed to create a proposal.
    proposal_init_fee: Balance,
    /// Voting result. `None` means the poll is still open.
    result: Option<ProposalId>,
    /// Epoch height when the contract is touched last time.
    last_epoch_height: EpochHeight,
}

impl Default for VotingContract {
    fn default() -> Self {
        env::panic(b"Voting contract should be initialized before usage")
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
impl VotingContract {
    #[init]
    pub fn new(description: String, threshold: Fraction, proposal_init_fee: U128) -> Self {
        VotingContract {
            description,
            proposals: Map::new(b"p".to_vec()),
            accounts: Map::new(b"a".to_vec()),
            account_stake: Map::new(b"s".to_vec()),
            next_proposal_id: U64(0),
            threshold,
            proposal_init_fee: proposal_init_fee.into(),
            result: None,
            last_epoch_height: 0,
        }
    }

    /// Submitting a new proposal
    #[payable]
    pub fn new_proposal(
        &mut self,
        description: String,
        metadata: String,
        expiration_height: u64,
    ) -> ProposalId {
        let cur_block_height = env::block_index();
        if self.result.is_some() {
            // This means the voting is already done.
            env::panic(format!("Voting is now closed.").as_bytes());
        }

        if env::attached_deposit() < self.proposal_init_fee {
            env::panic(
                format!(
                    "Creating a proposal costs {} but attached deposit is {}",
                    self.proposal_init_fee,
                    env::attached_deposit()
                )
                .as_bytes(),
            );
        }

        // check validity of the proposed height
        if expiration_height < cur_block_height + MIN_HEIGHT_HORIZON
            || expiration_height > cur_block_height + MAX_HEIGHT_HORIZON
        {
            let message = if expiration_height < cur_block_height + MIN_HEIGHT_HORIZON {
                format!(
                    "proposed height {} is below the smallest allowed height {}",
                    expiration_height,
                    cur_block_height + MIN_HEIGHT_HORIZON
                )
            } else {
                format!(
                    "proposed height {} is above the largest allowed height {}",
                    expiration_height,
                    cur_block_height + MAX_HEIGHT_HORIZON
                )
            };
            env::panic(message.as_bytes());
        }

        let result = self.next_proposal_id;
        let proposal = Proposal {
            description,
            metadata,
            expiration_height,
            votes: Map::new(format!("{}v", result.0).into_bytes()),
            account_stake: Map::new(format!("{}a", result.0).into_bytes()),
            total_voted_stake: 0,
            last_epoch_height: 0,
        };

        self.proposals.insert(&result, &proposal);
        self.next_proposal_id = U64(self.next_proposal_id.0 + 1);
        // Backdoor. The owner of the contract can set the result immediately.
        if env::current_account_id() == env::predecessor_account_id() {
            self.result = Some(result);
        }
        result
    }

    fn resolve_account_stake(&mut self) {
        let cur_epoch_height = env::epoch_height();
        if cur_epoch_height != self.last_epoch_height {
            for account_id in self.account_stake.keys().into_iter().collect::<Vec<_>>() {
                let old_account_stake = self.account_stake.remove(&account_id).unwrap();
                let account_current_stake = get_account_stake(&account_id);
                if old_account_stake != account_current_stake {
                    let mut account_voted_stake = self.accounts.remove(&account_id).unwrap();
                    if account_current_stake > 0 {
                        // TODO: use u256
                        account_voted_stake =
                            account_voted_stake * account_current_stake / old_account_stake;
                        self.accounts.insert(&account_id, &account_voted_stake);
                    }
                }
                if account_current_stake > 0 {
                    self.account_stake
                        .insert(&account_id, &account_current_stake);
                }
            }
            self.last_epoch_height = cur_epoch_height;
        }
    }

    fn resolve_proposal(&mut self, proposal_id: &ProposalId) {
        let mut proposal = match self.proposals.get(&proposal_id) {
            Some(p) => p,
            None => env::panic(format!("proposal {} doesn't exist", proposal_id.0).as_bytes()),
        };
        if proposal.expiration_height > env::block_index() {
            for (account_id, voted_stake) in proposal.votes.iter() {
                if let Some(voted_total_stake) = self.accounts.remove(&account_id) {
                    let new_total_voted_stake = voted_total_stake - voted_stake;
                    if new_total_voted_stake > 0 {
                        self.accounts.insert(&account_id, &new_total_voted_stake);
                    }
                }
            }
            self.proposals.remove(&proposal_id);
            return;
        }
        let cur_epoch_height = env::epoch_height();
        if cur_epoch_height != proposal.last_epoch_height {
            self.resolve_account_stake();
            let mut total_voted_stake = 0;
            for account_id in proposal.votes.keys().into_iter().collect::<Vec<_>>() {
                let prev_voted_stake = proposal.votes.remove(&account_id).unwrap();
                let current_stake = get_account_stake(&account_id);
                if current_stake > 0 {
                    let prev_stake = proposal.account_stake.get(&account_id).unwrap();
                    let cur_voted_stake = prev_voted_stake * current_stake / prev_stake;
                    proposal.account_stake.insert(&account_id, &current_stake);
                    proposal.votes.insert(&account_id, &cur_voted_stake);
                    total_voted_stake += cur_voted_stake
                }
            }
            proposal.total_voted_stake = total_voted_stake;
            proposal.last_epoch_height = cur_epoch_height;
            self.proposals.insert(&proposal_id, &proposal);
        }
    }

    /// Vote on a specific proposal with the given stake
    pub fn vote(&mut self, proposal_id: ProposalId, stake: Balance) {
        self.resolve_proposal(&proposal_id);
        let account_id = env::predecessor_account_id();
        let account_stake = get_account_stake(&account_id);
        if account_stake == 0 {
            env::panic(format!("account {} is not a validator", account_id).as_bytes());
        }
        if let Some(mut proposal) = self.proposals.remove(&proposal_id) {
            let mut total_voted = self.accounts.get(&account_id).unwrap_or_default();
            let proposal_voted_stake = proposal.votes.get(&account_id).unwrap_or_default();
            if total_voted + stake - proposal_voted_stake > account_stake {
                env::panic(
                    format!(
                        "account {} has a stake of {} but has already voted {} and tries to vote {}",
                        account_id,
                        account_stake,
                        total_voted - proposal_voted_stake,
                        stake
                    )
                        .as_bytes(),
                );
            }
            total_voted = total_voted + stake - proposal_voted_stake;
            proposal.total_voted_stake = proposal.total_voted_stake + stake - proposal_voted_stake;
            proposal.votes.insert(&account_id, &stake);
            self.proposals.insert(&proposal_id, &proposal);
            self.accounts.insert(&account_id, &total_voted);
            let epoch_total_stake = get_current_total_stake();
            if proposal_voted_stake
                > u128::from(self.threshold.numerator) * epoch_total_stake
                    / u128::from(self.threshold.denominator)
            {
                self.result = Some(proposal_id);
            }
        } else {
            env::panic(format!("proposal {} does not exist", proposal_id.0).as_bytes());
        }
    }

    pub fn get_proposal(&self, id: ProposalId) -> Option<Proposal> {
        self.proposals.get(&id)
    }

    pub fn get_result(&self) -> Option<ProposalId> {
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
        let mut contract = VotingContract::default();
        let id = contract.new_proposal("a great proposal".to_string(), 1000000);
        let proposal = Proposal {
            proposed_height: 1000000,
            description: "a great proposal".to_string(),
        };
        assert_eq!(Some(proposal), contract.get_proposal(id));
    }

    #[test]
    fn test_voting() {
        let mut contract = VotingContract::default();
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
        let mut contract = VotingContract::default();
        contract.new_proposal("a great proposal".to_string(), 1);
    }

    #[test]
    #[should_panic]
    fn test_illegal_proposal_height_too_large() {
        let context = get_context("bob.near".to_string());
        testing_env!(context);
        let mut contract = VotingContract::default();
        contract.new_proposal("a great proposal".to_string(), 10000000000000);
    }

    #[test]
    #[should_panic]
    fn test_vote_again_after_voting_ends() {
        let context = get_context("alice.near".to_string());
        testing_env!(context);
        let mut contract = VotingContract::default();
        let proposed_height = 10000000;
        contract.new_proposal("a great proposal".to_string(), proposed_height);
        assert_eq!(contract.result, Some(proposed_height));
        let context = get_context("bob.near".to_string());
        testing_env!(context);
        contract.new_proposal("a better proposal".to_string(), proposed_height + 1);
    }
}
