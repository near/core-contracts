//! A smart contract that allows tokens lockup.

use borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::json_types::{Base58PublicKey, U128, U64};
use near_sdk::{env, ext_contract, near_bindgen, AccountId, Promise, PromiseResult};
use serde::{Deserialize, Serialize};

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

pub type WrappedTimestamp = U64;
pub type WrappedBalance = U128;

pub type ProposalId = u64;
pub type VoteIndex = u64;

/// Method names allowed for the owner's access keys.
const OWNER_KEY_ALLOWED_METHODS: &[u8] =
    b"vote,select_staking_pool,unselect_staking_pool,deposit_to_staking_pool,withdraw_from_staking_pool,stake,unstake,check_transfers_vote,transfer";
/// Method names allowed for the NEAR Foundation access key in case of vesting schedule that
/// can be terminated by foundation.
const FOUNDATION_KEY_ALLOWED_METHODS: &[u8] =
    b"terminate_vesting,unstake_unvested,withdraw_unvested";

/// Indicates there are no deposit for a cross contract call for better readability.
const NO_DEPOSIT: u128 = 0;

pub mod gas {
    pub mod whitelist {
        /// Gas attached to the promise to check whether the given staking pool Account ID is
        /// whitelisted.
        /// Requires 100e12 (no external calls).
        pub const IS_WHITELISTED: u64 = 100_000_000_000_000;
    }

    pub mod staking_pool {
        /// The amount of gas required for a voting through a staking pool.
        /// Requires 100e12 for execution + 200e12 for attaching to a call on the voting contract.
        pub const VOTE: u64 = 300_000_000_000_000;

        /// The amount of gas required to get total user balance from the staking pool.
        /// Requires 100e12 for local processing.
        pub const GET_TOTAL_USER_BALANCE: u64 = 100_000_000_000_000;

        /// Gas attached to deposit call on the staking pool contract.
        /// Requires 100e12 for local updates.
        pub const DEPOSIT: u64 = 100_000_000_000_000;

        /// Gas attached to withdraw call on the staking pool contract.
        /// Requires 100e12 for execution + 200e12 for transferring amount to us.
        pub const WITHDRAW: u64 = 300_000_000_000_000;

        /// Gas attached to stake call on the staking pool contract.
        /// Requires 100e12 for execution + 200e12 for staking call.
        pub const STAKE: u64 = 300_000_000_000_000;

        /// Gas attached to unstake call on the staking pool contract.
        /// Requires 100e12 for execution + 200e12 for staking call.
        pub const UNSTAKE: u64 = 300_000_000_000_000;
    }

    pub mod voting {
        /// Gas attached to the promise to check whether transfers were enabled on the voting
        /// contract.
        /// Requires 100e12 (no external calls).
        pub const GET_RESULT: u64 = 100_000_000_000_000;
    }

    pub mod callbacks {
        /// Gas attached to the inner callback for processing whitelist check results.
        /// Requires 100e12 for local execution.
        pub const ON_WHITELIST_IS_WHITELISTED: u64 = 100_000_000_000_000;

        /// Gas attached to the inner callback for processing result of the call to get balance on
        /// the staking pool balance.
        /// Requires 100e12 for local updates.
        pub const ON_STAKING_POOL_GET_TOTAL_USER_BALANCE: u64 = 100_000_000_000_000;

        /// Gas attached to the inner callback for processing result of the deposit call to the
        /// staking pool.
        /// Requires 100e12 for local updates.
        pub const ON_STAKING_POOL_DEPOSIT: u64 = 100_000_000_000_000;

        /// Gas attached to the inner callback for processing result of the withdraw call to the
        /// staking pool.
        /// Requires 100e12 for local updates.
        pub const ON_STAKING_POOL_WITHDRAW: u64 = 100_000_000_000_000;

        /// Gas attached to the inner callback for processing result of the stake call to the
        /// staking pool.
        pub const ON_STAKING_POOL_STAKE: u64 = 100_000_000_000_000;

        /// Gas attached to the inner callback for processing result of the unstake call  to the
        /// staking pool.
        /// Requires 100e12 for local updates.
        pub const ON_STAKING_POOL_UNSTAKE: u64 = 100_000_000_000_000;

        /// Gas attached to the inner callback for processing result of the checking result for
        /// transfer voting call to the voting contract.
        /// Requires 100e12 for local updates.
        pub const ON_VOTING_GET_RESULT: u64 = 100_000_000_000_000;
    }
}

/// Contains information about token lockups.
#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
pub struct LockupInformation {
    /// The amount in yacto-NEAR tokens locked for this account.
    pub lockup_amount: WrappedBalance,
    /// The timestamp in nanoseconds when the lockup amount of tokens will be available.
    pub lockup_timestamp: WrappedTimestamp,

    /// Information about vesting if the lockup schedule includes vesting.
    /// `Some` means there is vesting information available.
    /// `None` means the lockup balance is unaffected by vesting.
    pub vesting_information: Option<VestingInformation>,
}

impl LockupInformation {
    pub fn assert_valid(&self) {
        assert!(
            self.lockup_amount.0 > 0,
            "Lockup amount has to be positive number"
        );
        match &self.vesting_information {
            Some(VestingInformation::Vesting(vesting_schedule)) => vesting_schedule.assert_valid(),
            Some(VestingInformation::Terminating(_termination_information)) => {
                panic!("The contract should not be initialized in termination stage")
            }
            None => (),
        };
    }
}

/// Describes the status of the interaction with the staking pool contract.
#[derive(BorshDeserialize, BorshSerialize, PartialEq)]
pub enum StakingStatus {
    /// There are no transactions in progress.
    Idle,
    /// There is a transaction in progress.
    Busy,
}

/// Contains information about current stake and delegation.
#[derive(BorshDeserialize, BorshSerialize)]
pub struct StakingInformation {
    /// The Account ID of the staking pool contract.
    pub staking_pool_account_id: AccountId,

    /// Contains status whether there is a transaction in progress.
    pub status: StakingStatus,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
pub struct VestingSchedule {
    /// The timestamp in nanosecond when the vesting starts. E.g. the start date of employment.
    pub vesting_start_timestamp: WrappedTimestamp,
    /// The timestamp in nanosecond when the first part of lockup tokens becomes vested.
    /// The remaining tokens will vest continuously until they are fully vested.
    /// Example: a 1 year of employment at which moment the 1/4 of tokens become vested.
    pub vesting_cliff_timestamp: WrappedTimestamp,
    /// The timestamp in nanosecond when the vesting ends.
    pub vesting_end_timestamp: WrappedTimestamp,
}

impl VestingSchedule {
    pub fn assert_valid(&self) {
        assert!(
            self.vesting_start_timestamp.0 <= self.vesting_cliff_timestamp.0,
            "Cliff timestamp can't be earlier than vesting start timestamp"
        );
        assert!(
            self.vesting_cliff_timestamp.0 <= self.vesting_end_timestamp.0,
            "Cliff timestamp can't be later than vesting end timestamp"
        );
    }
}

/// Contains information about vesting for contracts that contain vesting schedule and termination information.
#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
pub enum VestingInformation {
    /// The vesting is going on schedule.
    /// Once the vesting is completed `VestingInformation` is removed.
    Vesting(VestingSchedule),
    /// The information about the early termination of the vesting schedule.
    /// It means the termination of the vesting is currently in progress.
    /// Once the unvested amount is transferred out, `VestingInformation` is removed.
    Terminating(TerminationInformation),
}

/// Contains information about early termination of the vesting schedule.
#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
pub struct TerminationInformation {
    /// The amount of tokens that are unvested and has to be transferred back to NEAR Foundation.
    /// These tokens are effectively locked and can't be transferred out and can't be restaked.
    pub unvested_amount: WrappedBalance,
}

/// Contains information about voting on enabling transfers.
#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
pub struct TransferVotingInformation {
    /// The proposal ID to vote on transfers.
    pub transfer_proposal_id: ProposalId,

    /// Vote index indicating that the transfers are enabled.
    pub enable_transfers_vote_index: VoteIndex,

    /// Voting contract account ID
    pub voting_contract_account_id: AccountId,
}

impl TransferVotingInformation {
    pub fn assert_valid(&self) {
        assert!(
            env::is_valid_account_id(self.voting_contract_account_id.as_bytes()),
            "Voting contract account ID is invalid"
        );
    }
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct LockupContract {
    /// Information about lockup schedule and the amount.
    pub lockup_information: LockupInformation,

    /// Account ID of the staking pool whitelist contract.
    pub staking_pool_whitelist_account_id: AccountId,

    /// Information about staking and delegation.
    /// `Some` means the staking information is available and the staking pool contract is selected.
    /// `None` means there is no staking pool selected.
    pub staking_information: Option<StakingInformation>,

    /// Information about transfer voting. At the launch transfers are disabled, once transfers are
    /// enabled, they can't be disabled and don't need to be checked again.
    /// `Some` means transfers are disabled. `TransferVotingInformation` contains information
    /// required to check whether transfers were voted to be enabled.
    /// If transfers are disabled, every transfer attempt will try to first pull the results
    /// of transfer voting from the voting contract using transfer proposal ID.
    pub transfer_voting_information: Option<TransferVotingInformation>,
}

impl Default for LockupContract {
    fn default() -> Self {
        env::panic(b"The contract is not initialized.");
    }
}

fn assert_self() {
    assert_eq!(env::predecessor_account_id(), env::current_account_id());
}

fn is_promise_success() -> bool {
    assert_eq!(
        env::promise_results_count(),
        1,
        "Contract expected a result on the callback"
    );
    match env::promise_result(0) {
        PromiseResult::Successful(_) => true,
        _ => false,
    }
}

#[ext_contract(ext_staking_pool)]
pub trait ExtStakingPool {
    fn vote(&mut self, proposal_id: ProposalId, vote: VoteIndex);

    fn get_total_user_balance(&self, account_id: AccountId) -> U128;

    fn deposit(&mut self);

    fn withdraw(&mut self, amount: U128);

    fn stake(&mut self, amount: U128);

    fn unstake(&mut self, amount: U128);
}

#[ext_contract(ext_whitelist)]
pub trait ExtStakingPoolWhitelist {
    fn is_whitelisted(&self, staking_pool_account_id: AccountId) -> bool;
}

#[ext_contract(ext_voting)]
pub trait ExtVotingContract {
    fn get_result(&self, proposal_id: ProposalId) -> Option<VoteIndex>;
}

#[ext_contract(ext_self)]
pub trait ExtLockupContract {
    fn on_whitelist_is_whitelisted(
        &mut self,
        #[callback] is_whitelisted: bool,
        staking_pool_account_id: AccountId,
    ) -> bool;

    fn on_staking_pool_get_total_user_balance(&mut self, #[callback] total_balance: U128) -> bool;

    fn on_staking_pool_deposit(&mut self, amount: U128) -> bool;

    fn on_staking_pool_withdraw(&mut self, amount: U128) -> bool;

    fn on_staking_pool_stake(&mut self, amount: U128) -> bool;

    fn on_staking_pool_unstake(&mut self, amount: U128) -> bool;

    fn on_voting_get_result(&mut self, #[callback] vote_index: Option<VoteIndex>) -> bool;
}

#[near_bindgen]
impl LockupContract {
    /// Initializes lockup contract.
    /// - `lockup_information` - information about the lockup amount and the release timestamp.
    /// - `staking_pool_whitelist_account_id` - the Account ID of the staking pool whitelist contract.
    /// - `vesting_schedule` - if `Some` contains vesting schedule.
    /// - `transfer_voting_information` - if `Some` means transfers are disabled and can only be
    ///   enabled by voting on the proposal.
    /// - `owner_public_keys`
    #[init]
    pub fn new(
        lockup_information: LockupInformation,
        staking_pool_whitelist_account_id: AccountId,
        transfer_voting_information: Option<TransferVotingInformation>,
        owner_public_keys: Vec<Base58PublicKey>,
        foundation_public_keys: Vec<Base58PublicKey>,
    ) -> Self {
        assert!(!env::state_exists(), "The contract is already initialized");
        assert!(
            env::is_valid_account_id(staking_pool_whitelist_account_id.as_bytes()),
            "The staking pool whitelist account ID is invalid"
        );
        lockup_information.assert_valid();
        if !foundation_public_keys.is_empty() {
            assert!(
                lockup_information.vesting_information.is_some(),
                "Foundation keys can't be added without vesting schedule"
            )
        }
        assert!(
            !owner_public_keys.is_empty(),
            "At least one owner's public key has to be provided"
        );
        if let Some(transfer_voting_information) = transfer_voting_information.as_ref() {
            transfer_voting_information.assert_valid();
        }
        let account_id = env::current_account_id();
        for public_key in owner_public_keys {
            Promise::new(account_id.clone()).add_access_key(
                public_key.into(),
                0,
                account_id.clone(),
                OWNER_KEY_ALLOWED_METHODS.to_vec(),
            );
        }
        for public_key in foundation_public_keys {
            Promise::new(account_id.clone()).add_access_key(
                public_key.into(),
                0,
                account_id.clone(),
                FOUNDATION_KEY_ALLOWED_METHODS.to_vec(),
            );
        }
        Self {
            lockup_information,
            staking_information: None,
            staking_pool_whitelist_account_id,
            transfer_voting_information,
        }
    }

    /// OWNER'S METHOD
    /// Vote on given proposal ID with a selected vote index.
    /// The owner has to first delegate the stake to some staking pool contract before voting on
    /// a proposal.
    pub fn vote(&mut self, proposal_id: ProposalId, vote: VoteIndex) -> Promise {
        assert_self();
        self.assert_staking_pool_is_idle();
        env::log(format!("Voting for proposal {} with vote {}", proposal_id, vote).as_bytes());
        ext_staking_pool::vote(
            proposal_id,
            vote,
            &self
                .staking_information
                .as_ref()
                .unwrap()
                .staking_pool_account_id,
            NO_DEPOSIT,
            gas::staking_pool::VOTE,
        )
    }

    /// OWNER'S METHOD
    /// Selects staking pool contract at the given account ID. The staking pool first has to be
    /// checked against the staking pool whitelist contract.
    pub fn select_staking_pool(&mut self, staking_pool_account_id: AccountId) -> Promise {
        assert_self();
        assert!(
            env::is_valid_account_id(staking_pool_account_id.as_bytes()),
            "The staking pool account ID is invalid"
        );
        self.assert_staking_pool_is_not_selected();

        env::log(
            format!(
                "Selecting staking pool @{}. Going to check whitelist first.",
                staking_pool_account_id
            )
            .as_bytes(),
        );

        ext_whitelist::is_whitelisted(
            staking_pool_account_id.clone(),
            &self.staking_pool_whitelist_account_id,
            NO_DEPOSIT,
            gas::whitelist::IS_WHITELISTED,
        )
        .then(ext_self::on_whitelist_is_whitelisted(
            staking_pool_account_id,
            &env::current_account_id(),
            NO_DEPOSIT,
            gas::callbacks::ON_WHITELIST_IS_WHITELISTED,
        ))
    }

    /// OWNER'S METHOD
    /// Tries to unselect staking pool. It requires verifying that there are no deposits left on the
    /// current selected staking pool.
    pub fn unselect_staking_pool(&mut self) -> Promise {
        assert_self();
        self.assert_staking_pool_is_idle();

        env::log(
            format!(
                "Unselecting current staking pool @{}. Going to check current deposits first.",
                self.staking_information
                    .as_ref()
                    .unwrap()
                    .staking_pool_account_id
            )
            .as_bytes(),
        );

        self.set_staking_status(StakingStatus::Busy);

        ext_staking_pool::get_total_user_balance(
            env::current_account_id(),
            &self
                .staking_information
                .as_ref()
                .unwrap()
                .staking_pool_account_id,
            NO_DEPOSIT,
            gas::staking_pool::GET_TOTAL_USER_BALANCE,
        )
        .then(ext_self::on_staking_pool_get_total_user_balance(
            &env::current_account_id(),
            NO_DEPOSIT,
            gas::callbacks::ON_STAKING_POOL_GET_TOTAL_USER_BALANCE,
        ))
    }

    /// OWNER'S METHOD
    /// Deposits the given extra amount to the staking pool
    pub fn deposit_to_staking_pool(&mut self, amount: U128) -> Promise {
        assert_self();
        self.assert_staking_pool_is_idle();
        // TODO: Validate the extra amount

        env::log(
            format!(
                "Depositing {} to the staking pool @{}",
                amount.0,
                self.staking_information
                    .as_ref()
                    .unwrap()
                    .staking_pool_account_id
            )
            .as_bytes(),
        );

        self.set_staking_status(StakingStatus::Busy);

        ext_staking_pool::deposit(
            &self
                .staking_information
                .as_ref()
                .unwrap()
                .staking_pool_account_id,
            amount.0,
            gas::staking_pool::DEPOSIT,
        )
        .then(ext_self::on_staking_pool_deposit(
            amount,
            &env::current_account_id(),
            NO_DEPOSIT,
            gas::callbacks::ON_STAKING_POOL_DEPOSIT,
        ))
    }

    /// OWNER'S METHOD
    /// Withdraws the given amount from the staking pool
    pub fn withdraw_from_staking_pool(&mut self, amount: U128) -> Promise {
        assert_self();
        self.assert_staking_pool_is_idle();

        env::log(
            format!(
                "Withdrawing {} from the staking pool @{}",
                amount.0,
                self.staking_information
                    .as_ref()
                    .unwrap()
                    .staking_pool_account_id
            )
            .as_bytes(),
        );

        self.set_staking_status(StakingStatus::Busy);

        ext_staking_pool::withdraw(
            amount,
            &self
                .staking_information
                .as_ref()
                .unwrap()
                .staking_pool_account_id,
            NO_DEPOSIT,
            gas::staking_pool::WITHDRAW,
        )
        .then(ext_self::on_staking_pool_withdraw(
            amount,
            &env::current_account_id(),
            NO_DEPOSIT,
            gas::callbacks::ON_STAKING_POOL_WITHDRAW,
        ))
    }

    /// OWNER'S METHOD
    /// Stakes the given extra amount at the staking pool
    pub fn stake(&mut self, amount: U128) -> Promise {
        assert_self();
        self.assert_staking_pool_is_idle();
        // TODO: Validate the extra amount

        env::log(
            format!(
                "Staking {} at the staking pool @{}",
                amount.0,
                self.staking_information
                    .as_ref()
                    .unwrap()
                    .staking_pool_account_id
            )
            .as_bytes(),
        );

        self.set_staking_status(StakingStatus::Busy);

        ext_staking_pool::stake(
            amount,
            &self
                .staking_information
                .as_ref()
                .unwrap()
                .staking_pool_account_id,
            NO_DEPOSIT,
            gas::staking_pool::STAKE,
        )
        .then(ext_self::on_staking_pool_stake(
            amount,
            &env::current_account_id(),
            NO_DEPOSIT,
            gas::callbacks::ON_STAKING_POOL_STAKE,
        ))
    }

    /// OWNER'S METHOD
    /// Unstakes the given amount at the staking pool
    pub fn unstake(&mut self, amount: U128) -> Promise {
        assert_self();
        self.assert_staking_pool_is_idle();

        env::log(
            format!(
                "Unstaking {} at the staking pool @{}",
                amount.0,
                self.staking_information
                    .as_ref()
                    .unwrap()
                    .staking_pool_account_id
            )
            .as_bytes(),
        );

        self.set_staking_status(StakingStatus::Busy);

        ext_staking_pool::unstake(
            amount,
            &self
                .staking_information
                .as_ref()
                .unwrap()
                .staking_pool_account_id,
            NO_DEPOSIT,
            gas::staking_pool::UNSTAKE,
        )
        .then(ext_self::on_staking_pool_unstake(
            amount,
            &env::current_account_id(),
            NO_DEPOSIT,
            gas::callbacks::ON_STAKING_POOL_UNSTAKE,
        ))
    }

    /// OWNER'S METHOD
    pub fn check_transfers_vote(&mut self) -> Promise {
        assert_self();
        self.assert_transfers_disabled();

        let transfer_voting_information = self.transfer_voting_information.as_ref().unwrap();

        env::log(
            format!(
                "Checking that transfers are enabled (proposal {}) at the voting contract @{}",
                transfer_voting_information.transfer_proposal_id,
                transfer_voting_information.voting_contract_account_id,
            )
            .as_bytes(),
        );

        ext_voting::get_result(
            transfer_voting_information.transfer_proposal_id,
            &transfer_voting_information.voting_contract_account_id,
            NO_DEPOSIT,
            gas::voting::GET_RESULT,
        )
        .then(ext_self::on_voting_get_result(
            &env::current_account_id(),
            NO_DEPOSIT,
            gas::callbacks::ON_VOTING_GET_RESULT,
        ))
    }

    /// OWNER'S METHOD
    /// Transfers the given extra amount to the given receiver account ID.
    /// This requires transfers to be enabled within the voting contract.
    pub fn transfer(&mut self, amount: U128, receiver_id: AccountId) -> Promise {
        assert_self();
        assert!(
            env::is_valid_account_id(receiver_id.as_bytes()),
            "The receiver account ID is invalid"
        );
        self.assert_transfers_enabled();
        self.assert_no_staking_or_idle();
        // TODO: Verify transfer amount

        env::log(format!("Transferring {} to account @{}", amount.0, receiver_id).as_bytes());

        Promise::new(receiver_id).transfer(amount.0)
    }

    /*************/
    /* Callbacks */
    /*************/

    /// Called after a given `staking_pool_account_id` was checked in the whitelist.
    pub fn on_whitelist_is_whitelisted(
        &mut self,
        #[callback] is_whitelisted: bool,
        staking_pool_account_id: AccountId,
    ) -> bool {
        assert_self();
        assert!(
            is_whitelisted,
            "The given staking pool account ID is not whitelisted"
        );
        self.assert_staking_pool_is_not_selected();
        self.staking_information = Some(StakingInformation {
            staking_pool_account_id,
            status: StakingStatus::Idle,
        });
        true
    }

    /// Called after there was a request to unselect current staking pool.
    pub fn on_staking_pool_get_total_user_balance(
        &mut self,
        #[callback] total_balance: U128,
    ) -> bool {
        assert_self();
        if total_balance.0 > 0 {
            // There is still positive balance on the staking pool. Can't unselect the pool.
            self.set_staking_status(StakingStatus::Idle);
            false
        } else {
            self.staking_information = None;
            true
        }
    }

    /// Called after a deposit amount was transferred out of this account to the staking pool
    /// This method needs to update staking pool status.
    pub fn on_staking_pool_deposit(&mut self, amount: U128) -> bool {
        assert_self();

        let deposit_succeeded = is_promise_success();
        self.set_staking_status(StakingStatus::Idle);

        if deposit_succeeded {
            env::log(
                format!(
                    "The deposit of {} to @{} succeeded",
                    amount.0,
                    self.staking_information
                        .as_ref()
                        .unwrap()
                        .staking_pool_account_id
                )
                .as_bytes(),
            );
        } else {
            env::log(
                format!(
                    "The deposit of {} to @{} has failed",
                    amount.0,
                    self.staking_information
                        .as_ref()
                        .unwrap()
                        .staking_pool_account_id
                )
                .as_bytes(),
            );
        }
        deposit_succeeded
    }

    /// Called after the given amount was requested to transfer out from the staking pool to this
    /// account.
    /// This method needs to update staking pool status.
    pub fn on_staking_pool_withdraw(&mut self, amount: U128) -> bool {
        assert_self();

        let withdraw_succeeded = is_promise_success();
        self.set_staking_status(StakingStatus::Idle);

        if withdraw_succeeded {
            env::log(
                format!(
                    "The withdrawal of {} from @{} succeeded",
                    amount.0,
                    self.staking_information
                        .as_ref()
                        .unwrap()
                        .staking_pool_account_id
                )
                .as_bytes(),
            );
        } else {
            env::log(
                format!(
                    "The withdrawal of {} from @{} failed",
                    amount.0,
                    self.staking_information
                        .as_ref()
                        .unwrap()
                        .staking_pool_account_id
                )
                .as_bytes(),
            );
        }
        withdraw_succeeded
    }

    /// Called after the extra amount stake was staked in the staking pool contract.
    /// This method needs to update staking pool status.
    pub fn on_staking_pool_stake(&mut self, amount: U128) -> bool {
        assert_self();

        let stake_succeeded = is_promise_success();
        self.set_staking_status(StakingStatus::Idle);

        if stake_succeeded {
            env::log(
                format!(
                    "Staking of {} at @{} succeeded",
                    amount.0,
                    self.staking_information
                        .as_ref()
                        .unwrap()
                        .staking_pool_account_id
                )
                .as_bytes(),
            );
        } else {
            env::log(
                format!(
                    "Staking {} at @{} has failed",
                    amount.0,
                    self.staking_information
                        .as_ref()
                        .unwrap()
                        .staking_pool_account_id
                )
                .as_bytes(),
            );
        }
        stake_succeeded
    }

    /// Called after the extra amount stake was staked in the staking pool contract.
    /// This method needs to update staking pool status.
    pub fn on_staking_pool_unstake(&mut self, amount: U128) -> bool {
        assert_self();

        let unstake_succeeded = is_promise_success();
        self.set_staking_status(StakingStatus::Idle);

        if unstake_succeeded {
            env::log(
                format!(
                    "Unstaking of {} at @{} succeeded",
                    amount.0,
                    self.staking_information
                        .as_ref()
                        .unwrap()
                        .staking_pool_account_id
                )
                .as_bytes(),
            );
        } else {
            env::log(
                format!(
                    "Unstaking {} at @{} has failed",
                    amount.0,
                    self.staking_information
                        .as_ref()
                        .unwrap()
                        .staking_pool_account_id
                )
                .as_bytes(),
            );
        }
        unstake_succeeded
    }

    /// Called after the extra amount stake was staked in the staking pool contract.
    /// This method needs to update staking pool status.
    pub fn on_voting_get_result(&mut self, #[callback] vote_index: Option<VoteIndex>) -> bool {
        assert_self();
        self.assert_transfers_disabled();

        let expected_vote_index = self
            .transfer_voting_information
            .as_ref()
            .unwrap()
            .enable_transfers_vote_index;

        if let Some(vote_index) = vote_index {
            assert_eq!(vote_index, expected_vote_index, "The enable transfers proposal has been resolved to a different vote. Transfers will never be enabled.");
            env::log(b"Transfers has been successfully enabled");
            self.transfer_voting_information = None;
            true
        } else {
            env::log(b"Voting on enabling transfers doesn't have a majority vote yet");
            false
        }
    }

    /********************/
    /* Internal methods */
    /********************/

    fn set_staking_status(&mut self, status: StakingStatus) {
        self.staking_information
            .as_mut()
            .expect("Staking pool should be selected")
            .status = status;
    }

    fn assert_transfers_enabled(&self) {
        assert!(
            self.transfer_voting_information.is_none(),
            "Transfers are disabled"
        );
    }

    fn assert_transfers_disabled(&self) {
        assert!(
            self.transfer_voting_information.is_some(),
            "Transfers are already enabled"
        );
    }

    fn assert_no_staking_or_idle(&self) {
        if let Some(staking_information) = &self.staking_information {
            match staking_information.status {
                StakingStatus::Idle => (),
                StakingStatus::Busy => {
                    env::panic(b"Contract is currently busy with another operation")
                }
            };
        }
    }

    fn assert_staking_pool_is_idle(&self) {
        assert!(
            self.staking_information.is_some(),
            "Staking pool is not selected"
        );
        match self.staking_information.as_ref().unwrap().status {
            StakingStatus::Idle => (),
            StakingStatus::Busy => env::panic(b"Contract is currently busy with another operation"),
        };
    }

    fn assert_staking_pool_is_not_selected(&self) {
        assert!(
            self.staking_information.is_none(),
            "Staking pool is already selected"
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use super::*;

    use near_sdk::{testing_env, MockedBlockchain, VMContext};
    use std::convert::TryInto;
    /*
    fn basic_setup() -> (VMContext, LockupContract) {
        let context = get_context(
            system_account(),
            to_yocto(LOCKUP_NEAR),
            0,
            to_ts(GENESIS_TIME_IN_DAYS),
            false,
        );
        testing_env!(context.clone());
        // Contract Setup:
        // - Now is genesis time.
        // - Lockup amount is 1000 near tokens.
        // - Lockup for 1 year.
        // - Owner has 2 keys
        let contract = LockupContract::new(
            to_yocto(LOCKUP_NEAR).into(),
            to_ts(GENESIS_TIME_IN_DAYS + YEAR).into(),
            vec![
                public_key(1).try_into().unwrap(),
                public_key(2).try_into().unwrap(),
            ],
        );
        (context, contract)
    }

    #[test]
    fn test_basic() {
        let (mut context, contract) = basic_setup();
        // Checking initial values at genesis time
        context.is_view = true;
        testing_env!(context.clone());

        assert_eq!(contract.get_transferrable().0, 0);

        // Checking values in 1 day after genesis time
        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + 1);

        assert_eq!(contract.get_transferrable().0, 0);

        // Checking values next day after lockup timestamp
        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR + 1);
        testing_env!(context.clone());

        assert_almost_eq(contract.get_transferrable().0, to_yocto(LOCKUP_NEAR));
    }

    #[test]
    fn test_transferrable_with_different_stakes() {
        let (mut context, contract) = basic_setup();

        // Staking everything at the genesis
        context.account_locked_balance = to_yocto(999);
        context.account_balance = to_yocto(1);

        // Checking values in 1 day after genesis time
        context.is_view = true;

        for stake in &[1, 10, 100, 500, 999, 1001, 1005, 1100, 1500, 1999, 3000] {
            let stake = *stake;
            context.account_locked_balance = to_yocto(stake);
            let balance_near = std::cmp::max(1000u128.saturating_sub(stake), 1);
            context.account_balance = to_yocto(balance_near);
            let extra_balance_near = stake + balance_near - 1000;

            context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + 1);
            testing_env!(context.clone());

            assert_eq!(contract.get_transferrable().0, to_yocto(extra_balance_near));

            // Checking values next day after lockup timestamp
            context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR + 1);
            testing_env!(context.clone());

            assert_almost_eq(
                contract.get_transferrable().0,
                to_yocto(LOCKUP_NEAR + extra_balance_near),
            );
        }
    }

    #[test]
    fn test_transfer_call_by_owner() {
        let (mut context, mut contract) = basic_setup();
        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR + 1);
        context.is_view = true;
        testing_env!(context.clone());
        assert_almost_eq(contract.get_transferrable().0, to_yocto(LOCKUP_NEAR));

        context.predecessor_account_id = account_owner();
        context.signer_account_id = account_owner();
        context.signer_account_pk = public_key(1);
        context.is_view = false;
        testing_env!(context.clone());

        assert_eq!(env::account_balance(), to_yocto(LOCKUP_NEAR));
        contract.transfer(to_yocto(100).into(), non_owner());
        assert_almost_eq(env::account_balance(), to_yocto(LOCKUP_NEAR - 100));
    }

    #[test]
    fn test_stake_call_by_owner() {
        let (mut context, mut contract) = basic_setup();
        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR + 1);
        context.is_view = true;
        testing_env!(context.clone());
        assert_almost_eq(contract.get_transferrable().0, to_yocto(LOCKUP_NEAR));

        context.predecessor_account_id = account_owner();
        context.signer_account_id = account_owner();
        context.signer_account_pk = public_key(1);
        context.is_view = false;
        testing_env!(context.clone());

        assert_eq!(env::account_balance(), to_yocto(LOCKUP_NEAR));
        contract.stake(to_yocto(100).into(), public_key(10).try_into().unwrap());
        assert_almost_eq(env::account_balance(), to_yocto(LOCKUP_NEAR));
    }

    #[test]
    fn test_transfer_by_non_owner() {
        let (mut context, mut contract) = basic_setup();

        context.predecessor_account_id = non_owner();
        context.signer_account_id = non_owner();
        context.signer_account_pk = public_key(5);
        testing_env!(context.clone());

        std::panic::catch_unwind(move || {
            contract.transfer(to_yocto(100).into(), non_owner());
        })
        .unwrap_err();
    }

    #[test]
    fn test_stake_by_non_owner() {
        let (mut context, mut contract) = basic_setup();

        context.predecessor_account_id = non_owner();
        context.signer_account_id = non_owner();
        context.signer_account_pk = public_key(5);
        testing_env!(context.clone());

        std::panic::catch_unwind(move || {
            contract.stake(to_yocto(100).into(), public_key(4).try_into().unwrap());
        })
        .unwrap_err();
    }
    */
}
