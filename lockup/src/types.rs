use borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::json_types::{U128, U64};
use near_sdk::{env, AccountId, BlockHeight};
use serde::{Deserialize, Serialize};
use uint::construct_uint;

construct_uint! {
    /// 256-bit unsigned integer.
    // TODO: Revert back to 4 once wasm/wasmer bug is fixed.
    // See https://github.com/wasmerio/wasmer/issues/1429
    pub struct U256(8);
}
/// Timestamp in nanosecond wrapped into a struct for JSON serialization as a string.
pub type WrappedTimestamp = U64;
/// Duration in nanosecond wrapped into a struct for JSON serialization as a string.
pub type WrappedDuration = U64;
/// Balance wrapped into a struct for JSON serialization as a string.
pub type WrappedBalance = U128;

pub type ProposalId = u64;

/// Contains information about token lockups.
#[derive(BorshDeserialize, BorshSerialize, Deserialize, Serialize)]
pub struct LockupInformation {
    /// The amount in yacto-NEAR tokens locked for this account.
    pub lockup_amount: WrappedBalance,
    /// The lockup duration in nanoseconds from the moment when transfers are enabled to unlock the
    /// lockup amount of tokens.
    pub lockup_duration: WrappedDuration,
    /// The timestamp when the transfers were enabled.
    /// If None, the transfers are not enabled yet.
    pub lockup_timestamp: Option<WrappedTimestamp>,

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

    /// The amount of tokens that were deposited from this account to the staking pool.
    /// NOTE: The unstaked amount on the staking pool might be higher due to staking rewards.
    pub deposit_amount: WrappedBalance,
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

/// Describes the status of transactions with the staking pool contract or terminated unvesting
/// amount withdrawal.
#[derive(
    BorshDeserialize, BorshSerialize, Deserialize, Serialize, PartialEq, Copy, Clone, Debug,
)]
pub enum TerminationStatus {
    /// Initial stage of the termination in case there are deficit on the account.
    VestingTerminatedWithDeficit,
    /// A transaction to unstake everything is in progress.
    UnstakingInProgress,
    /// The transaction to unstake everything from the staking pool has completed.
    EverythingUnstaked,
    /// A transaction to withdraw everything from the staking pool is in progress.
    WithdrawingFromStakingPoolInProgress,
    /// Everything is withdrawn from the staking pool. Ready to withdraw out of the account.
    ReadyToWithdraw,
    /// A transaction to withdraw tokens from the account is in progress.
    WithdrawingFromAccountInProgress,
}

/// Contains information about early termination of the vesting schedule.
#[derive(BorshDeserialize, BorshSerialize, Deserialize, Serialize)]
pub struct TerminationInformation {
    /// The amount of tokens that are unvested and has to be transferred back to NEAR Foundation.
    /// These tokens are effectively locked and can't be transferred out and can't be restaked.
    pub unvested_amount: WrappedBalance,

    /// The status of the withdrawal. When the unvested amount is in progress of withdrawal the
    /// status will be marked as busy, to avoid withdrawing the funds twice.
    pub status: TerminationStatus,
}

/// Contains information about voting on enabling transfers.
#[derive(BorshDeserialize, BorshSerialize, Deserialize, Serialize)]
pub struct TransferVotingInformation {
    /// The proposal ID which is expected to be voted on.
    pub transfer_proposal_id: ProposalId,

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

/// Contains information about poll result.
#[derive(Deserialize)]
pub struct PollResult {
    /// The proposal ID that was voted in.
    pub proposal_id: ProposalId,
    /// The timestamp when the proposal was voted in.
    pub timestamp: WrappedTimestamp,
    /// The block height when the proposal was voted in.
    pub block_height: BlockHeight,
}
