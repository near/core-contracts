//! A smart contract that allows tokens lockup.

use borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::json_types::{Base58PublicKey, U128, U64};
use near_sdk::{env, ext_contract, near_bindgen, AccountId, Promise, PromiseResult};
use serde::{Deserialize, Serialize};
use uint::construct_uint;

construct_uint! {
    /// 256-bit unsigned integer.
    pub struct U256(4);
}

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
    b"terminate_vesting,resolve_deficit,withdraw_unvested_amount";

/// Indicates there are no deposit for a cross contract call for better readability.
const NO_DEPOSIT: u128 = 0;

/// The contract keeps at least 30 NEAR in the account to avoid being transferred out to cover
/// contract code storage and some internal state.
const MIN_BALANCE_FOR_STORAGE: u128 = 30_000_000_000_000_000_000_000_000;

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

        /// Gas attached to the inner callback for processing result of the withdrawal of the
        /// terminated unvested balance.
        /// Requires 100e12 for local updates.
        pub const ON_WITHDRAW_UNVESTED_AMOUNT: u64 = 100_000_000_000_000;
    }
}

/// Contains information about token lockups.
#[derive(BorshDeserialize, BorshSerialize, Deserialize, Serialize)]
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

/// Describes the status of transactions with the staking pool contract or terminated unvesting
/// amount withdrawal.
#[derive(BorshDeserialize, BorshSerialize, Deserialize, Serialize, PartialEq)]
pub enum TransactionStatus {
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
    pub status: TransactionStatus,

    /// The minimum amount of tokens that were deposited from this account to the staking pool.
    /// The actual amount might be higher due to stake rewards. This contract can't track stake
    /// rewards on the staking pool reliably without querying it.
    pub deposit_amount: WrappedBalance,

    /// The minimum amount of tokens that were staked on the staking pool.
    /// The actual amount might be higher due to stake rewards. This contract can't track stake
    /// rewards on the staking pool reliably without querying it.
    pub stake_amount: WrappedBalance,
}

#[derive(BorshDeserialize, BorshSerialize, Deserialize, Serialize)]
pub struct VestingSchedule {
    /// The timestamp in nanosecond when the vesting starts. E.g. the start date of employment.
    pub start_timestamp: WrappedTimestamp,
    /// The timestamp in nanosecond when the first part of lockup tokens becomes vested.
    /// The remaining tokens will vest continuously until they are fully vested.
    /// Example: a 1 year of employment at which moment the 1/4 of tokens become vested.
    pub cliff_timestamp: WrappedTimestamp,
    /// The timestamp in nanosecond when the vesting ends.
    pub end_timestamp: WrappedTimestamp,
}

impl VestingSchedule {
    pub fn assert_valid(&self) {
        assert!(
            self.start_timestamp.0 <= self.cliff_timestamp.0,
            "Cliff timestamp can't be earlier than vesting start timestamp"
        );
        assert!(
            self.cliff_timestamp.0 <= self.end_timestamp.0,
            "Cliff timestamp can't be later than vesting end timestamp"
        );
        assert!(
            self.start_timestamp.0 < self.end_timestamp.0,
            "The total vesting time should be positive"
        );
    }
}

/// Contains information about vesting for contracts that contain vesting schedule and termination information.
#[derive(BorshDeserialize, BorshSerialize, Deserialize, Serialize)]
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
#[derive(BorshDeserialize, BorshSerialize, Deserialize, Serialize)]
pub struct TerminationInformation {
    /// The amount of tokens that are unvested and has to be transferred back to NEAR Foundation.
    /// These tokens are effectively locked and can't be transferred out and can't be restaked.
    pub unvested_amount: WrappedBalance,

    /// The status of the withdrawal. When the unvested amount is in progress of withdrawal the
    /// status will be marked as busy, to avoid withdrawing the funds twice.
    pub status: TransactionStatus,
}

/// Contains information about voting on enabling transfers.
#[derive(BorshDeserialize, BorshSerialize, Deserialize, Serialize)]
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

    fn get_total_user_balance(&self, account_id: AccountId) -> WrappedBalance;

    fn deposit(&mut self);

    fn withdraw(&mut self, amount: WrappedBalance);

    fn stake(&mut self, amount: WrappedBalance);

    fn unstake(&mut self, amount: WrappedBalance);
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

    fn on_staking_pool_get_total_user_balance(
        &mut self,
        #[callback] total_balance: WrappedBalance,
    ) -> bool;

    fn on_staking_pool_deposit(&mut self, amount: WrappedBalance) -> bool;

    fn on_staking_pool_withdraw(&mut self, amount: WrappedBalance) -> bool;

    fn on_staking_pool_stake(&mut self, amount: WrappedBalance) -> bool;

    fn on_staking_pool_unstake(&mut self, amount: WrappedBalance) -> bool;

    fn on_voting_get_result(&mut self, #[callback] vote_index: Option<VoteIndex>) -> bool;

    fn on_withdraw_unvested_amount(
        &mut self,
        amount: WrappedBalance,
        receiver_id: AccountId,
    ) -> bool;
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

    /*******************/
    /* Owner's Methods */
    /*******************/

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
        self.assert_no_deficit();
        // NOTE: This is best effort checks. There is still some balance might be left on the
        // staking pool, which is validated below.
        assert_eq!(
            self.staking_information.as_ref().unwrap().deposit_amount.0,
            0,
            "There is still a deposit on the staking pool"
        );
        assert_eq!(
            self.staking_information.as_ref().unwrap().stake_amount.0,
            0,
            "There is still a stake on the staking pool"
        );

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

        self.set_staking_status(TransactionStatus::Busy);

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
    pub fn deposit_to_staking_pool(&mut self, amount: WrappedBalance) -> Promise {
        assert_self();
        assert!(amount.0 > 0, "Amount should be positive");
        self.assert_staking_pool_is_idle();
        self.assert_no_deficit();
        assert!(
            self.get_liquid_balance().0 >= amount.0,
            "The balance that can be deposited to the staking pool is lower than the extra amount"
        );

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

        self.set_staking_status(TransactionStatus::Busy);

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
    pub fn withdraw_from_staking_pool(&mut self, amount: WrappedBalance) -> Promise {
        assert_self();
        assert!(amount.0 > 0, "Amount should be positive");
        self.assert_staking_pool_is_idle();
        let deficit = self.get_terminated_unvested_balance_deficit().0;
        if deficit > 0 {
            // The owner should not withdraw less than the deficit to avoid blocking operations
            // on the pool.
            assert!(
                amount.0 > deficit,
                "Can't withdraw less than the terminated unvested balance deficit"
            );
            // Need to verify that the withdrawal amount is not larger than known unstaked balance.
            // It's possible that the withdrawal can still succeed otherwise, but since it would
            // require blocking the staking pool is better to avoid such operation until the deficit
            // is resolved.
            assert!(
                self.get_known_unstaked_balance().0 >= amount.0,
                "Trying to withdraw more than known unstaked balance during terminated unvested balance deficit"
            );
        }

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

        self.set_staking_status(TransactionStatus::Busy);

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
    pub fn stake(&mut self, amount: WrappedBalance) -> Promise {
        assert_self();
        assert!(amount.0 > 0, "Amount should be positive");
        self.assert_staking_pool_is_idle();
        self.assert_no_deficit();

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

        self.set_staking_status(TransactionStatus::Busy);

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
    pub fn unstake(&mut self, amount: WrappedBalance) -> Promise {
        assert_self();
        assert!(amount.0 > 0, "Amount should be positive");
        self.assert_staking_pool_is_idle();
        let deficit = self.get_terminated_unvested_balance_deficit().0;
        if deficit > 0 {
            // During deficit the contract only allows to unstake known amount to avoid blocking the
            // contract with failed call.
            assert!(
                amount.0 <= self.staking_information.as_ref().unwrap().stake_amount.0,
                "Can't unstake the amount larger than known staked amount during terminated unvested balance deficit"
            );
            // During deficit the contract shouldn't allow to unstake more tokens than needed to
            // avoid blocking the pool.
            let unstaked_balance = self.get_known_unstaked_balance().0;
            assert!(
                deficit > unstaked_balance,
                "Can't unstake more tokens until the terminated unvested balance deficit is returned back to the account"
            );
            let need_to_unstake = deficit - unstaked_balance;
            assert!(
                amount.0 >= need_to_unstake,
                format!(
                    "Can't unstake less than the required amount of {} to cover terminated unvested balance deficit of {}",
                    need_to_unstake,
                    deficit
                )
            )
        }

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

        self.set_staking_status(TransactionStatus::Busy);

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
    /// Calls voting contract to validate if the transfers were enabled by voting. Once transfers
    /// are enabled, they can't be disabled anymore.
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
    pub fn transfer(&mut self, amount: WrappedBalance, receiver_id: AccountId) -> Promise {
        assert_self();
        assert!(amount.0 > 0, "Amount should be positive");
        assert!(
            env::is_valid_account_id(receiver_id.as_bytes()),
            "The receiver account ID is invalid"
        );
        self.assert_transfers_enabled();
        self.assert_no_staking_or_idle();
        assert!(
            self.get_liquid_owners_balance().0 >= amount.0,
            "The available liquid balance is smaller than the requested transfer amount"
        );

        env::log(format!("Transferring {} to account @{}", amount.0, receiver_id).as_bytes());

        Promise::new(receiver_id).transfer(amount.0)
    }

    /************************/
    /* Foundation's Methods */
    /************************/

    /// FOUNDATION'S METHOD
    /// Terminates vesting schedule and locks the remaining unvested amount.
    pub fn terminate_vesting(&mut self) {
        assert_self();
        assert_eq!(
            self.get_terminated_unvested_balance().0,
            0,
            "Vesting has been already terminated"
        );
        let unvested_amount = self.get_unvested_amount();
        assert!(unvested_amount.0 > 0, "The account is fully vested");

        env::log(
            format!(
                "Terminating vesting. The remaining unvested balance is {}",
                unvested_amount.0
            )
            .as_bytes(),
        );

        self.lockup_information.vesting_information =
            Some(VestingInformation::Terminating(TerminationInformation {
                unvested_amount,
                status: TransactionStatus::Idle,
            }));
    }

    /// FOUNDATION'S METHOD
    /// When the vesting is terminated and there are deficit of the tokens on the account, the
    /// deficit amount of tokens has to be unstaked and withdrawn from the staking pool.
    pub fn resolve_deficit(&mut self) -> Promise {
        assert_self();
        self.assert_staking_pool_is_idle();
        self.assert_termination_is_idle();

        let deficit = self.get_terminated_unvested_balance_deficit().0;
        assert!(deficit > 0, "There are no unvested balance deficit");

        let unstaked_balance = self.get_known_unstaked_balance().0;

        if unstaked_balance < deficit {
            let need_to_unstake = deficit - unstaked_balance;
            env::log(
                format!(
                    "Trying to unstake {} to be able to withdraw termination unvested balance deficit of {}",
                    need_to_unstake,
                    deficit,
                )
                    .as_bytes(),
            );
            self.unstake(need_to_unstake.into())
        } else {
            env::log(
                format!(
                    "Trying to withdraw {} to cover the termination unvested balance deficit",
                    deficit
                )
                .as_bytes(),
            );

            self.withdraw_from_staking_pool(deficit.into())
        }
    }

    /// FOUNDATION'S METHOD
    /// Withdraws the unvested amount from the early termination of the vesting schedule.
    pub fn withdraw_unvested_amount(&mut self, receiver_id: AccountId) -> Promise {
        assert_self();
        assert!(
            env::is_valid_account_id(receiver_id.as_bytes()),
            "The receiver account ID is invalid"
        );
        self.assert_termination_is_idle();

        let amount = self.get_terminated_unvested_balance();
        assert!(
            self.get_account_balance().0 >= amount.0,
            "The account doesn't have enough balance to withdraw the unvested amount"
        );

        env::log(
            format!(
                "Withdrawing {} terminated unvested balance to account @{}",
                amount.0, receiver_id
            )
            .as_bytes(),
        );

        self.set_terminating_status(TransactionStatus::Busy);

        Promise::new(receiver_id.clone()).transfer(amount.0).then(
            ext_self::on_withdraw_unvested_amount(
                amount,
                receiver_id,
                &env::current_account_id(),
                NO_DEPOSIT,
                gas::callbacks::ON_WITHDRAW_UNVESTED_AMOUNT,
            ),
        )
    }

    /***********/
    /* Getters */
    /***********/

    /// The amount of tokens that can be deposited to the staking pool or transferred out.
    /// It excludes tokens that are locked due to early termination of the vesting schedule.
    pub fn get_liquid_balance(&self) -> WrappedBalance {
        self.get_account_balance()
            .0
            .saturating_sub(self.get_terminated_unvested_balance().0)
            .into()
    }

    /// The amount of tokens that are not going to be vested, because the vesting schedule was
    /// terminated earlier.
    pub fn get_terminated_unvested_balance(&self) -> WrappedBalance {
        if let Some(VestingInformation::Terminating(TerminationInformation {
            unvested_amount,
            ..
        })) = &self.lockup_information.vesting_information
        {
            *unvested_amount
        } else {
            0.into()
        }
    }

    /// The amount of tokens missing from the account balance that are required to cover
    /// the unvested balance from the early-terminated vesting schedule.
    pub fn get_terminated_unvested_balance_deficit(&self) -> WrappedBalance {
        self.get_terminated_unvested_balance()
            .0
            .saturating_sub(self.get_account_balance().0)
            .into()
    }

    /// Get the amount of tokens that are locked in this account due to lockup or vesting.
    pub fn get_locked_amount(&self) -> WrappedBalance {
        if self.lockup_information.lockup_timestamp.0 > env::block_timestamp() {
            // The entire balance is still locked before the lockup timestamp.
            return self.lockup_information.lockup_amount;
        }
        self.get_unvested_amount()
    }

    /// Get the amount of tokens that are locked in this account due to vesting.
    pub fn get_unvested_amount(&self) -> WrappedBalance {
        let block_timestamp = env::block_timestamp();
        let lockup_amount = self.lockup_information.lockup_amount.0;
        if let Some(vesting_information) = &self.lockup_information.vesting_information {
            match vesting_information {
                VestingInformation::Vesting(vesting_schedule) => {
                    if block_timestamp < vesting_schedule.cliff_timestamp.0 {
                        // Before the cliff, nothing is vested
                        lockup_amount.into()
                    } else if block_timestamp >= vesting_schedule.end_timestamp.0 {
                        // After the end, everything is vested
                        0.into()
                    } else {
                        // cannot overflow since block_timestamp >= vesting_schedule.end_timestamp
                        let time_left =
                            U256::from(vesting_schedule.end_timestamp.0 - block_timestamp);
                        // The total time is positive. Checked at the contract initialization.
                        let total_time = U256::from(
                            vesting_schedule.end_timestamp.0 - vesting_schedule.start_timestamp.0,
                        );
                        let unvested_amount = U256::from(lockup_amount) * time_left / total_time;
                        // The unvested amount can't be larger than lockup_amount because the
                        // time_left is smaller than total_time.
                        unvested_amount.as_u128().into()
                    }
                }
                VestingInformation::Terminating(termination_information) => {
                    // It's safe to subtract, because terminated amount can't be larger.
                    (lockup_amount - termination_information.unvested_amount.0).into()
                }
            }
        } else {
            // Everything is vested and unlocked
            0.into()
        }
    }

    /// The balance of the account owner. It includes vested and extra tokens that may have been
    /// deposited to this account.
    /// NOTE: Some of this tokens may be deposited to the staking pool.
    /// Also it doesn't account for tokens locked for the contract storage.
    pub fn get_owners_balance(&self) -> WrappedBalance {
        (env::account_balance() + self.get_known_deposited_balance().0)
            .saturating_sub(self.get_locked_amount().0)
            .into()
    }

    /// The amount of tokens the owner can transfer now from the account.
    pub fn get_liquid_owners_balance(&self) -> WrappedBalance {
        std::cmp::min(self.get_owners_balance().0, self.get_liquid_balance().0).into()
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
            status: TransactionStatus::Idle,
            deposit_amount: 0.into(),
            stake_amount: 0.into(),
        });
        true
    }

    /// Called after there was a request to unselect current staking pool.
    pub fn on_staking_pool_get_total_user_balance(
        &mut self,
        #[callback] total_balance: WrappedBalance,
    ) -> bool {
        assert_self();
        if total_balance.0 > 0 {
            // There is still positive balance on the staking pool. Can't unselect the pool.
            self.set_staking_status(TransactionStatus::Idle);
            false
        } else {
            self.staking_information = None;
            true
        }
    }

    /// Called after a deposit amount was transferred out of this account to the staking pool
    /// This method needs to update staking pool status.
    pub fn on_staking_pool_deposit(&mut self, amount: WrappedBalance) -> bool {
        assert_self();

        let deposit_succeeded = is_promise_success();
        self.set_staking_status(TransactionStatus::Idle);

        if deposit_succeeded {
            self.staking_information.as_mut().unwrap().deposit_amount.0 += amount.0;
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
    pub fn on_staking_pool_withdraw(&mut self, amount: WrappedBalance) -> bool {
        assert_self();

        let withdraw_succeeded = is_promise_success();
        self.set_staking_status(TransactionStatus::Idle);

        if withdraw_succeeded {
            {
                let staking_information = self.staking_information.as_mut().unwrap();
                // Due to staking rewards the deposit amount can become negative.
                staking_information.deposit_amount.0 = staking_information
                    .deposit_amount
                    .0
                    .saturating_sub(amount.0);
            }
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
    pub fn on_staking_pool_stake(&mut self, amount: WrappedBalance) -> bool {
        assert_self();

        let stake_succeeded = is_promise_success();
        self.set_staking_status(TransactionStatus::Idle);

        if stake_succeeded {
            {
                let staking_information = self.staking_information.as_mut().unwrap();
                staking_information.stake_amount.0 += amount.0;
                staking_information.deposit_amount.0 = std::cmp::max(
                    staking_information.deposit_amount.0,
                    staking_information.stake_amount.0,
                );
            }
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
    pub fn on_staking_pool_unstake(&mut self, amount: WrappedBalance) -> bool {
        assert_self();

        let unstake_succeeded = is_promise_success();
        self.set_staking_status(TransactionStatus::Idle);

        if unstake_succeeded {
            {
                let staking_information = self.staking_information.as_mut().unwrap();
                if amount.0 > staking_information.stake_amount.0 {
                    staking_information.deposit_amount.0 +=
                        amount.0 - staking_information.stake_amount.0;
                    staking_information.stake_amount.0 = 0;
                } else {
                    staking_information.stake_amount.0 -= amount.0;
                }
            }
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

    /// Called after the foundation tried to withdraw the unvested amount from the account.
    pub fn on_withdraw_unvested_amount(
        &mut self,
        amount: WrappedBalance,
        receiver_id: AccountId,
    ) -> bool {
        assert_self();

        let withdraw_succeeded = is_promise_success();
        if withdraw_succeeded {
            self.lockup_information.vesting_information = None;
            env::log(
                format!(
                    "The withdrawal of the terminated unvested amount of {} to @{} succeeded",
                    amount.0, receiver_id,
                )
                .as_bytes(),
            );
        } else {
            self.set_terminating_status(TransactionStatus::Idle);
            env::log(
                format!(
                    "The withdrawal of the terminated unvested amount of {} to @{} failed",
                    amount.0, receiver_id,
                )
                .as_bytes(),
            );
        }
        withdraw_succeeded
    }

    /********************/
    /* Internal methods */
    /********************/

    /// The balance of the account excluding the storage staking balance.
    /// NOTE: The storage staking balance can't be transferred out without deleting this contract.
    fn get_account_balance(&self) -> WrappedBalance {
        env::account_balance()
            .saturating_sub(MIN_BALANCE_FOR_STORAGE)
            .into()
    }

    fn get_known_unstaked_balance(&self) -> WrappedBalance {
        self.staking_information
            .as_ref()
            .map(|info| {
                // Known deposit is always greater or equal to the known stake.
                info.deposit_amount.0 - info.stake_amount.0
            })
            .unwrap_or(0)
            .into()
    }

    /// The amount of tokens that were deposited to the staking pool.
    /// NOTE: The actual balance can be larger than this known deposit balance due to staking
    /// rewards acquired on the staking pool.
    fn get_known_deposited_balance(&self) -> WrappedBalance {
        self.staking_information
            .as_ref()
            .map(|info| info.deposit_amount.0)
            .unwrap_or(0)
            .into()
    }

    fn set_staking_status(&mut self, status: TransactionStatus) {
        self.staking_information
            .as_mut()
            .expect("Staking pool should be selected")
            .status = status;
    }

    fn set_terminating_status(&mut self, status: TransactionStatus) {
        if let Some(VestingInformation::Terminating(termination_information)) =
            self.lockup_information.vesting_information.as_mut()
        {
            termination_information.status = status;
        } else {
            unreachable!("The vesting information is not at the terminating stage");
        }
    }

    fn assert_no_deficit(&self) {
        assert_eq!(
            self.get_terminated_unvested_balance_deficit().0, 0,
            "All normal staking pool operations are blocked until the terminated unvested balance deficit is returned to the account"
        );
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
                TransactionStatus::Idle => (),
                TransactionStatus::Busy => {
                    env::panic(b"Contract is currently busy with another operation")
                }
            };
        }
    }

    fn assert_termination_is_idle(&self) {
        if let Some(VestingInformation::Terminating(termination_information)) =
            &self.lockup_information.vesting_information
        {
            match termination_information.status {
                TransactionStatus::Idle => (),
                TransactionStatus::Busy => {
                    env::panic(b"Contract is currently busy with termination withdrawal")
                }
            };
        } else {
            env::panic(b"There are no termination in progress");
        }
    }

    fn assert_staking_pool_is_idle(&self) {
        assert!(
            self.staking_information.is_some(),
            "Staking pool is not selected"
        );
        match self.staking_information.as_ref().unwrap().status {
            TransactionStatus::Idle => (),
            TransactionStatus::Busy => {
                env::panic(b"Contract is currently busy with another operation")
            }
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

    pub type AccountId = String;

    pub const LOCKUP_NEAR: u128 = 1000;
    pub const GENESIS_TIME_IN_DAYS: u64 = 500;
    pub const YEAR: u64 = 365;
    pub const ALMOST_HALF_YEAR: u64 = YEAR / 2;

    pub fn system_account() -> AccountId {
        "system".to_string()
    }

    pub fn account_owner() -> AccountId {
        "account_owner".to_string()
    }

    pub fn non_owner() -> AccountId {
        "non_owner".to_string()
    }

    pub fn to_yocto(near_balance: u128) -> u128 {
        near_balance * 10u128.pow(24)
    }

    pub fn to_ts(num_days: u64) -> u64 {
        // 2018-08-01 UTC in nanoseconds
        1533081600_000_000_000 + num_days * 86400_000_000_000
    }

    pub fn assert_almost_eq_with_max_delta(left: u128, right: u128, max_delta: u128) {
        assert!(
            std::cmp::max(left, right) - std::cmp::min(left, right) < max_delta,
            format!(
                "Left {} is not even close to Right {} within delta {}",
                left, right, max_delta
            )
        );
    }

    pub fn assert_almost_eq(left: u128, right: u128) {
        assert_almost_eq_with_max_delta(left, right, to_yocto(10));
    }

    pub fn get_context(
        predecessor_account_id: AccountId,
        account_balance: u128,
        account_locked_balance: u128,
        block_timestamp: u64,
        is_view: bool,
    ) -> VMContext {
        VMContext {
            current_account_id: account_owner(),
            signer_account_id: predecessor_account_id.clone(),
            signer_account_pk: vec![0, 1, 2],
            predecessor_account_id,
            input: vec![],
            block_index: 1,
            block_timestamp,
            epoch_height: 1,
            account_balance,
            account_locked_balance,
            storage_usage: 10u64.pow(6),
            attached_deposit: 0,
            prepaid_gas: 10u64.pow(15),
            random_seed: vec![0, 1, 2],
            is_view,
            output_data_receivers: vec![],
        }
    }

    fn public_key(byte_val: u8) -> Base58PublicKey {
        let mut pk = vec![byte_val; 33];
        pk[0] = 0;
        Base58PublicKey(pk)
    }

    fn lockup_only_setup() -> (VMContext, LockupContract) {
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
            LockupInformation {
                lockup_amount: to_yocto(LOCKUP_NEAR).into(),
                lockup_timestamp: to_ts(GENESIS_TIME_IN_DAYS + YEAR).into(),
                vesting_information: None,
            },
            AccountId::from("whitelist"),
            None,
            vec![public_key(1), public_key(2)],
            vec![],
        );
        (context, contract)
    }

    #[test]
    fn test_lockup_only_basic() {
        let (mut context, contract) = lockup_only_setup();
        // Checking initial values at genesis time
        context.is_view = true;
        testing_env!(context.clone());

        assert_eq!(contract.get_owners_balance().0, 0);

        // Checking values in 1 day after genesis time
        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + 1);

        assert_eq!(contract.get_owners_balance().0, 0);

        // Checking values next day after lockup timestamp
        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR + 1);
        testing_env!(context.clone());

        assert_almost_eq(contract.get_owners_balance().0, to_yocto(LOCKUP_NEAR));
    }

    /*
    #[test]
    fn test_lockup_only_transferrable_with_different_stakes() {
        let (mut context, contract) = lockup_only_setup();

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

            assert_eq!(
                contract.get_owners_balance().0,
                to_yocto(extra_balance_near)
            );

            // Checking values next day after lockup timestamp
            context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR + 1);
            testing_env!(context.clone());

            assert_almost_eq(
                contract.get_owners_balance().0,
                to_yocto(LOCKUP_NEAR + extra_balance_near),
            );
        }
    }
    */

    #[test]
    fn test_lockup_only_transfer_call_by_owner() {
        let (mut context, mut contract) = lockup_only_setup();
        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR + 1);
        context.is_view = true;
        testing_env!(context.clone());
        assert_almost_eq(contract.get_owners_balance().0, to_yocto(LOCKUP_NEAR));

        context.predecessor_account_id = account_owner();
        context.signer_account_id = account_owner();
        context.signer_account_pk = public_key(1).try_into().unwrap();
        context.is_view = false;
        testing_env!(context.clone());

        assert_eq!(env::account_balance(), to_yocto(LOCKUP_NEAR));
        contract.transfer(to_yocto(100).into(), non_owner());
        assert_almost_eq(env::account_balance(), to_yocto(LOCKUP_NEAR - 100));
    }

    /*
    #[test]
    fn test_lockup_only_stake_call_by_owner() {
        let (mut context, mut contract) = lockup_only_setup();
        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR + 1);
        context.is_view = true;
        testing_env!(context.clone());
        assert_almost_eq(contract.get_owners_balance().0, to_yocto(LOCKUP_NEAR));

        context.predecessor_account_id = account_owner();
        context.signer_account_id = account_owner();
        context.signer_account_pk = public_key(1).try_into().unwrap();
        context.is_view = false;
        testing_env!(context.clone());

        assert_eq!(env::account_balance(), to_yocto(LOCKUP_NEAR));
        contract.stake(to_yocto(100).into(), public_key(10).try_into().unwrap());
        assert_almost_eq(env::account_balance(), to_yocto(LOCKUP_NEAR));
    }

    #[test]
    fn test_lockup_only_transfer_by_non_owner() {
        let (mut context, mut contract) = lockup_only_setup();

        context.predecessor_account_id = non_owner();
        context.signer_account_id = non_owner();
        context.signer_account_pk = public_key(5).try_into().unwrap();
        testing_env!(context.clone());

        std::panic::catch_unwind(move || {
            contract.transfer(to_yocto(100).into(), non_owner());
        })
        .unwrap_err();
    }

    #[test]
    fn test_lockup_only_stake_by_non_owner() {
        let (mut context, mut contract) = lockup_only_setup();

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
