//! A smart contract that allows tokens to be locked up.

use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::json_types::Base58PublicKey;
use near_sdk::{env, ext_contract, near_bindgen, AccountId};

pub mod types;
pub use crate::types::*;

pub mod utils;
pub use crate::utils::*;

pub mod owner_callbacks;
pub use crate::owner_callbacks::*;

pub mod foundation;
pub use crate::foundation::*;

pub mod foundation_callbacks;
pub use crate::foundation_callbacks::*;

pub mod gas;

pub mod getters;
pub use crate::getters::*;

pub mod internal;
pub use crate::internal::*;

pub mod owner;
pub use crate::owner::*;

#[global_allocator]
static ALLOC: near_sdk::wee_alloc::WeeAlloc = near_sdk::wee_alloc::WeeAlloc::INIT;

/// Indicates there are no deposit for a cross contract call for better readability.
const NO_DEPOSIT: u128 = 0;

/// The contract keeps at least 35 NEAR in the account to avoid being transferred out to cover
/// contract code storage and some internal state.
const MIN_BALANCE_FOR_STORAGE: u128 = 35_000_000_000_000_000_000_000_000;

#[ext_contract(ext_staking_pool)]
pub trait ExtStakingPool {
    fn get_account_staked_balance(&self, account_id: AccountId) -> WrappedBalance;

    fn get_account_unstaked_balance(&self, account_id: AccountId) -> WrappedBalance;

    fn get_account_total_balance(&self, account_id: AccountId) -> WrappedBalance;

    fn deposit(&mut self);

    fn deposit_and_stake(&mut self);

    fn withdraw(&mut self, amount: WrappedBalance);

    fn stake(&mut self, amount: WrappedBalance);

    fn unstake(&mut self, amount: WrappedBalance);

    fn unstake_all(&mut self);
}

#[ext_contract(ext_whitelist)]
pub trait ExtStakingPoolWhitelist {
    fn is_whitelisted(&self, staking_pool_account_id: AccountId) -> bool;
}

#[ext_contract(ext_transfer_poll)]
pub trait ExtTransferPoll {
    fn get_result(&self) -> Option<PollResult>;
}

#[ext_contract(ext_self_owner)]
pub trait ExtLockupContractOwner {
    fn on_whitelist_is_whitelisted(
        &mut self,
        #[callback] is_whitelisted: bool,
        staking_pool_account_id: AccountId,
    ) -> bool;

    fn on_staking_pool_deposit(&mut self, amount: WrappedBalance) -> bool;

    fn on_staking_pool_deposit_and_stake(&mut self, amount: WrappedBalance) -> bool;

    fn on_staking_pool_withdraw(&mut self, amount: WrappedBalance) -> bool;

    fn on_staking_pool_stake(&mut self, amount: WrappedBalance) -> bool;

    fn on_staking_pool_unstake(&mut self, amount: WrappedBalance) -> bool;

    fn on_staking_pool_unstake_all(&mut self) -> bool;

    fn on_get_result_from_transfer_poll(&mut self, #[callback] poll_result: PollResult) -> bool;

    fn on_get_account_total_balance(&mut self, #[callback] total_balance: WrappedBalance);

    fn on_get_account_unstaked_balance_to_withdraw_by_owner(
        &mut self,
        #[callback] unstaked_balance: WrappedBalance,
    );
}

#[ext_contract(ext_self_foundation)]
pub trait ExtLockupContractFoundation {
    fn on_withdraw_unvested_amount(
        &mut self,
        amount: WrappedBalance,
        receiver_id: AccountId,
    ) -> bool;

    fn on_get_account_staked_balance_to_unstake(
        &mut self,
        #[callback] staked_balance: WrappedBalance,
    );

    fn on_staking_pool_unstake_for_termination(&mut self, amount: WrappedBalance) -> bool;

    fn on_get_account_unstaked_balance_to_withdraw(
        &mut self,
        #[callback] unstaked_balance: WrappedBalance,
    );

    fn on_staking_pool_withdraw_for_termination(&mut self, amount: WrappedBalance) -> bool;
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct LockupContract {
    /// The account ID of the owner.
    pub owner_account_id: AccountId,

    /// Information about lockup schedule and the amount.
    pub lockup_information: LockupInformation,

    /// Information about vesting including schedule or termination status.
    pub vesting_information: VestingInformation,

    /// Account ID of the staking pool whitelist contract.
    pub staking_pool_whitelist_account_id: AccountId,

    /// Information about staking and delegation.
    /// `Some` means the staking information is available and the staking pool contract is selected.
    /// `None` means there is no staking pool selected.
    pub staking_information: Option<StakingInformation>,

    /// The account ID that the NEAR Foundation, that has the ability to terminate vesting.
    pub foundation_account_id: Option<AccountId>,
}

impl Default for LockupContract {
    fn default() -> Self {
        env::panic(b"The contract is not initialized.");
    }
}

#[near_bindgen]
impl LockupContract {
    /// Requires 25 TGas (1 * BASE_GAS)
    ///
    /// Initializes lockup contract.
    /// - `owner_account_id` - the account ID of the owner.  Only this account can call owner's
    ///    methods on this contract.
    /// - `lockup_duration` - the duration in nanoseconds of the lockup period from the moment
    ///    the transfers are enabled.
    /// - `lockup_timestamp` - the optional absolute lockup timestamp in nanoseconds which locks
    ///    the tokens until this timestamp passes.
    /// - `transfers_information` - the information about the transfers. Either transfers are
    ///    already enabled, then it contains the timestamp when they were enabled. Or the transfers
    ///    are currently disabled and it contains the account ID of the transfer poll contract.
    /// - `vesting_schedule` - if present, describes the vesting schedule for employees. Vesting
    ///    schedule affects the amount of tokens the NEAR Foundation will get in case of
    ///    employment termination as well as the amount of tokens available for transfer by
    ///    the employee.
    /// - `release_duration` - is the duration when the full lockup amount will be available.
    ///    The tokens are linearly released from the moment transfers are enabled. If it's used
    ///    in addition to the vesting schedule, then the amount of tokens available to transfer
    ///    is subject to the minimum between vested tokens and released tokens.
    /// - `staking_pool_whitelist_account_id` - the Account ID of the staking pool whitelist contract.
    /// - `foundation_account_id` - the account ID of the NEAR Foundation, that has the ability to
    ///    terminate vesting schedule.
    #[init]
    pub fn new(
        owner_account_id: AccountId,
        lockup_duration: WrappedDuration,
        lockup_timestamp: Option<WrappedTimestamp>,
        transfers_information: TransfersInformation,
        vesting_schedule: Option<VestingSchedule>,
        release_duration: Option<WrappedDuration>,
        staking_pool_whitelist_account_id: AccountId,
        foundation_account_id: Option<AccountId>,
    ) -> Self {
        assert!(!env::state_exists(), "The contract is already initialized");
        assert!(
            env::is_valid_account_id(owner_account_id.as_bytes()),
            "The account ID of the owner is invalid"
        );
        assert!(
            env::is_valid_account_id(staking_pool_whitelist_account_id.as_bytes()),
            "The staking pool whitelist account ID is invalid"
        );
        if foundation_account_id.is_some() {
            assert!(
                vesting_schedule.is_some(),
                "Foundation account can't be added without vesting schedule"
            )
        }
        if let TransfersInformation::TransfersDisabled {
            transfer_poll_account_id,
        } = &transfers_information
        {
            assert!(
                env::is_valid_account_id(transfer_poll_account_id.as_bytes()),
                "The transfer poll account ID is invalid"
            );
        }
        let lockup_information = LockupInformation {
            lockup_amount: env::account_balance(),
            termination_withdrawn_tokens: 0,
            lockup_duration: lockup_duration.0,
            release_duration: release_duration.map(|d| d.0),
            lockup_timestamp: lockup_timestamp.map(|d| d.0),
            transfers_information,
        };
        let vesting_information = if let Some(vesting_schedule) = vesting_schedule {
            vesting_schedule.assert_valid();
            VestingInformation::Vesting(vesting_schedule)
        } else {
            VestingInformation::None
        };

        Self {
            owner_account_id,
            lockup_information,
            vesting_information,
            staking_information: None,
            staking_pool_whitelist_account_id,
            foundation_account_id,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use super::*;

    use near_sdk::{testing_env, MockedBlockchain, PromiseResult, VMContext};
    use std::convert::TryInto;

    mod test_utils;
    use test_utils::*;

    pub type AccountId = String;

    fn basic_context() -> VMContext {
        get_context(
            system_account(),
            to_yocto(LOCKUP_NEAR),
            0,
            to_ts(GENESIS_TIME_IN_DAYS),
            false,
        )
    }

    fn new_vesting_schedule(offset_in_days: u64) -> Option<VestingSchedule> {
        Some(VestingSchedule {
            start_timestamp: to_ts(GENESIS_TIME_IN_DAYS - YEAR + offset_in_days).into(),
            cliff_timestamp: to_ts(GENESIS_TIME_IN_DAYS + offset_in_days).into(),
            end_timestamp: to_ts(GENESIS_TIME_IN_DAYS + YEAR * 3 + offset_in_days).into(),
        })
    }

    fn new_contract(
        transfers_enabled: bool,
        vesting_schedule: Option<VestingSchedule>,
        release_duration: Option<WrappedDuration>,
        foundation_account: bool,
    ) -> LockupContract {
        let lockup_start_information = if transfers_enabled {
            TransfersInformation::TransfersEnabled {
                transfers_timestamp: to_ts(GENESIS_TIME_IN_DAYS).into(),
            }
        } else {
            TransfersInformation::TransfersDisabled {
                transfer_poll_account_id: AccountId::from("transfers"),
            }
        };
        let foundation_account_id = if foundation_account {
            Some(account_foundation())
        } else {
            None
        };
        LockupContract::new(
            account_owner(),
            to_nanos(YEAR).into(),
            None,
            lockup_start_information,
            vesting_schedule,
            release_duration,
            AccountId::from("whitelist"),
            foundation_account_id,
        )
    }

    fn lockup_only_setup() -> (VMContext, LockupContract) {
        let context = basic_context();
        testing_env!(context.clone());
        let contract = new_contract(true, None, None, false);
        (context, contract)
    }

    #[test]
    fn test_lockup_only_basic() {
        let (mut context, contract) = lockup_only_setup();
        // Checking initial values at genesis time
        context.is_view = true;
        testing_env!(context.clone());

        assert_eq!(contract.get_owners_balance().0, 0);
        assert_eq!(contract.get_locked_vested_amount().0, to_yocto(LOCKUP_NEAR));

        // Checking values in 1 day after genesis time
        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + 1);

        assert_eq!(contract.get_owners_balance().0, 0);

        // Checking values next day after lockup timestamp
        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR + 1);
        testing_env!(context.clone());

        assert_almost_eq(contract.get_owners_balance().0, to_yocto(LOCKUP_NEAR));
    }

    #[test]
    fn test_add_full_access_key() {
        let (mut context, mut contract) = lockup_only_setup();
        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR);
        context.predecessor_account_id = account_owner();
        context.signer_account_id = account_owner();
        context.signer_account_pk = public_key(1).try_into().unwrap();
        testing_env!(context.clone());

        contract.add_full_access_key(public_key(4));
    }

    #[test]
    #[should_panic(expected = "Can only be called by the owner")]
    fn test_call_by_non_owner() {
        let (mut context, mut contract) = lockup_only_setup();
        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR);
        context.predecessor_account_id = non_owner();
        context.signer_account_id = non_owner();
        testing_env!(context.clone());

        contract.select_staking_pool(AccountId::from("staking_pool"));
    }

    #[test]
    #[should_panic(expected = "No NEAR Foundation account is specified in the contract")]
    fn test_no_foundation_call() {
        let (mut context, mut contract) = lockup_only_setup();
        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR);
        context.predecessor_account_id = non_owner();
        context.signer_account_id = non_owner();
        testing_env!(context.clone());

        contract.terminate_vesting();
    }

    #[test]
    #[should_panic(expected = "Can only be called by NEAR Foundation")]
    fn test_call_by_non_foundation() {
        let mut context = basic_context();
        testing_env!(context.clone());
        let mut contract = new_contract(true, new_vesting_schedule(0), None, true);
        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR);
        context.predecessor_account_id = non_owner();
        context.signer_account_id = non_owner();
        testing_env!(context.clone());

        contract.terminate_vesting();
    }

    #[test]
    #[should_panic(expected = "Transfers are disabled")]
    fn test_transfers_not_enabled() {
        let mut context = basic_context();
        testing_env!(context.clone());
        let mut contract = new_contract(false, None, None, false);
        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR + 1);
        context.predecessor_account_id = account_owner();
        context.signer_account_id = account_owner();
        context.signer_account_pk = public_key(1).try_into().unwrap();
        context.is_view = false;
        testing_env!(context.clone());

        contract.transfer(to_yocto(100).into(), non_owner());
    }

    #[test]
    fn test_enable_transfers() {
        let mut context = basic_context();
        testing_env!(context.clone());
        let mut contract = new_contract(false, None, None, false);
        context.is_view = true;
        testing_env!(context.clone());
        assert!(!contract.are_transfers_enabled());

        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR + 1);
        context.predecessor_account_id = account_owner();
        context.signer_account_id = account_owner();
        context.signer_account_pk = public_key(1).try_into().unwrap();
        context.is_view = false;
        testing_env!(context.clone());

        contract.check_transfers_vote();

        let poll_result = Some(to_ts(GENESIS_TIME_IN_DAYS + 10).into());
        context.predecessor_account_id = lockup_account();
        // NOTE: Unit tests don't need to read the content of the promise result. So here we don't
        // have to pass serialized result from the transfer poll.
        testing_env_with_promise_results(context.clone(), PromiseResult::Successful(vec![]));
        assert!(contract.on_get_result_from_transfer_poll(poll_result));

        context.is_view = true;
        testing_env!(context.clone());
        // Not unlocked yet
        assert_eq!(contract.get_owners_balance().0, 0);
        assert!(contract.are_transfers_enabled());

        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR + 10);
        testing_env!(context.clone());
        // Unlocked yet
        assert_eq!(
            contract.get_owners_balance().0,
            to_yocto(LOCKUP_NEAR).into()
        );

        context.is_view = false;
        context.predecessor_account_id = account_owner();
        testing_env!(context.clone());
        contract.transfer(to_yocto(100).into(), non_owner());
    }

    #[test]
    fn test_check_transfers_vote_false() {
        let mut context = basic_context();
        testing_env!(context.clone());
        let mut contract = new_contract(false, None, None, false);
        context.is_view = true;
        testing_env!(context.clone());
        assert!(!contract.are_transfers_enabled());

        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR + 1);
        context.predecessor_account_id = account_owner();
        context.signer_account_id = account_owner();
        context.signer_account_pk = public_key(1).try_into().unwrap();
        context.is_view = false;
        testing_env!(context.clone());

        contract.check_transfers_vote();

        let poll_result = None;
        // NOTE: Unit tests don't need to read the content of the promise result. So here we don't
        // have to pass serialized result from the transfer poll.
        context.predecessor_account_id = lockup_account();
        testing_env_with_promise_results(context.clone(), PromiseResult::Successful(vec![]));
        assert!(!contract.on_get_result_from_transfer_poll(poll_result));

        context.is_view = true;
        testing_env!(context.clone());
        // Not enabled
        assert!(!contract.are_transfers_enabled());
    }

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

    #[test]
    #[should_panic(expected = "Staking pool is not selected")]
    fn test_staking_pool_is_not_selected() {
        let (mut context, mut contract) = lockup_only_setup();
        context.predecessor_account_id = account_owner();
        context.signer_account_id = account_owner();
        context.signer_account_pk = public_key(2).try_into().unwrap();

        let amount = to_yocto(LOCKUP_NEAR - 100);
        testing_env!(context.clone());
        contract.deposit_to_staking_pool(amount.into());
    }

    #[test]
    fn test_staking_pool_success() {
        let (mut context, mut contract) = lockup_only_setup();
        context.predecessor_account_id = account_owner();
        context.signer_account_id = account_owner();
        context.signer_account_pk = public_key(2).try_into().unwrap();

        // Selecting staking pool
        let staking_pool = "staking_pool".to_string();
        testing_env!(context.clone());
        contract.select_staking_pool(staking_pool.clone());

        context.predecessor_account_id = lockup_account();
        testing_env_with_promise_results(
            context.clone(),
            PromiseResult::Successful(b"true".to_vec()),
        );
        contract.on_whitelist_is_whitelisted(true, staking_pool.clone());

        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_staking_pool_account_id(), Some(staking_pool));
        assert_eq!(contract.get_known_deposited_balance().0, 0);
        context.is_view = false;

        // Deposit to the staking_pool
        let amount = to_yocto(LOCKUP_NEAR - 100);
        context.predecessor_account_id = account_owner();
        testing_env!(context.clone());
        contract.deposit_to_staking_pool(amount.into());
        context.account_balance = env::account_balance();
        assert_eq!(context.account_balance, to_yocto(LOCKUP_NEAR) - amount);

        context.predecessor_account_id = lockup_account();
        testing_env_with_promise_results(context.clone(), PromiseResult::Successful(vec![]));
        contract.on_staking_pool_deposit(amount.into());
        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_known_deposited_balance().0, amount);
        context.is_view = false;

        // Staking on the staking pool
        context.predecessor_account_id = account_owner();
        testing_env!(context.clone());
        contract.stake(amount.into());

        context.predecessor_account_id = lockup_account();
        testing_env_with_promise_results(context.clone(), PromiseResult::Successful(vec![]));
        contract.on_staking_pool_stake(amount.into());

        // Assuming there are 20 NEAR tokens in rewards. Unstaking.
        let unstake_amount = amount + to_yocto(20);
        context.predecessor_account_id = account_owner();
        testing_env!(context.clone());
        contract.unstake(unstake_amount.into());

        context.predecessor_account_id = lockup_account();
        testing_env_with_promise_results(context.clone(), PromiseResult::Successful(vec![]));
        contract.on_staking_pool_unstake(unstake_amount.into());

        // Withdrawing
        context.predecessor_account_id = account_owner();
        testing_env!(context.clone());
        contract.withdraw_from_staking_pool(unstake_amount.into());
        context.account_balance += unstake_amount;

        context.predecessor_account_id = lockup_account();
        testing_env_with_promise_results(context.clone(), PromiseResult::Successful(vec![]));
        contract.on_staking_pool_withdraw(unstake_amount.into());
        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_known_deposited_balance().0, 0);
        context.is_view = false;

        // Unselecting staking pool
        context.predecessor_account_id = account_owner();
        testing_env!(context.clone());
        contract.unselect_staking_pool();
        assert_eq!(contract.get_staking_pool_account_id(), None);
    }

    #[test]
    fn test_staking_pool_refresh_balance() {
        let (mut context, mut contract) = lockup_only_setup();
        context.predecessor_account_id = account_owner();
        context.signer_account_id = account_owner();
        context.signer_account_pk = public_key(2).try_into().unwrap();

        // Selecting staking pool
        let staking_pool = "staking_pool".to_string();
        testing_env!(context.clone());
        contract.select_staking_pool(staking_pool.clone());

        context.predecessor_account_id = lockup_account();
        testing_env_with_promise_results(
            context.clone(),
            PromiseResult::Successful(b"true".to_vec()),
        );
        contract.on_whitelist_is_whitelisted(true, staking_pool.clone());

        // Deposit to the staking_pool
        let amount = to_yocto(LOCKUP_NEAR - 100);
        context.predecessor_account_id = account_owner();
        testing_env!(context.clone());
        contract.deposit_to_staking_pool(amount.into());
        context.account_balance = env::account_balance();
        assert_eq!(context.account_balance, to_yocto(LOCKUP_NEAR) - amount);

        context.predecessor_account_id = lockup_account();
        testing_env_with_promise_results(context.clone(), PromiseResult::Successful(vec![]));
        contract.on_staking_pool_deposit(amount.into());

        // Staking on the staking pool
        context.predecessor_account_id = account_owner();
        testing_env!(context.clone());
        contract.stake(amount.into());

        context.predecessor_account_id = lockup_account();
        testing_env_with_promise_results(context.clone(), PromiseResult::Successful(vec![]));
        contract.on_staking_pool_stake(amount.into());

        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, 0);
        assert_eq!(contract.get_liquid_owners_balance().0, 0);
        assert_eq!(contract.get_known_deposited_balance().0, amount);
        context.is_view = false;

        // Assuming there are 20 NEAR tokens in rewards. Refreshing balance.
        let total_balance = amount + to_yocto(20);
        context.predecessor_account_id = account_owner();
        testing_env!(context.clone());
        contract.refresh_staking_pool_balance();

        // In unit tests, the following call ignores the promise value, because it's passed directly.
        context.predecessor_account_id = lockup_account();
        testing_env_with_promise_results(context.clone(), PromiseResult::Successful(vec![]));
        contract.on_get_account_total_balance(total_balance.into());

        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_known_deposited_balance().0, total_balance);
        assert_eq!(contract.get_owners_balance().0, to_yocto(20));
        assert_eq!(contract.get_liquid_owners_balance().0, to_yocto(20));
        context.is_view = false;

        // Withdrawing these tokens
        context.predecessor_account_id = account_owner();
        testing_env!(context.clone());
        let transfer_amount = to_yocto(15);
        contract.transfer(transfer_amount.into(), non_owner());
        context.account_balance = env::account_balance();

        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_known_deposited_balance().0, total_balance);
        assert_eq!(contract.get_owners_balance().0, to_yocto(5));
        assert_eq!(contract.get_liquid_owners_balance().0, to_yocto(5));
        context.is_view = false;
    }

    #[test]
    #[should_panic(expected = "Staking pool is already selected")]
    fn test_staking_pool_selected_again() {
        let (mut context, mut contract) = lockup_only_setup();
        context.predecessor_account_id = account_owner();
        context.signer_account_id = account_owner();
        context.signer_account_pk = public_key(2).try_into().unwrap();

        // Selecting staking pool
        let staking_pool = "staking_pool".to_string();
        testing_env!(context.clone());
        contract.select_staking_pool(staking_pool.clone());

        context.predecessor_account_id = lockup_account();
        testing_env_with_promise_results(
            context.clone(),
            PromiseResult::Successful(b"true".to_vec()),
        );
        contract.on_whitelist_is_whitelisted(true, staking_pool.clone());

        // Selecting another staking pool
        context.predecessor_account_id = account_owner();
        testing_env!(context.clone());
        contract.select_staking_pool("staking_pool_2".to_string());
    }

    #[test]
    #[should_panic(expected = "The given staking pool account ID is not whitelisted")]
    fn test_staking_pool_not_whitelisted() {
        let (mut context, mut contract) = lockup_only_setup();
        context.predecessor_account_id = account_owner();
        context.signer_account_id = account_owner();
        context.signer_account_pk = public_key(2).try_into().unwrap();

        // Selecting staking pool
        let staking_pool = "staking_pool".to_string();
        testing_env!(context.clone());
        contract.select_staking_pool(staking_pool.clone());

        context.predecessor_account_id = lockup_account();
        context.predecessor_account_id = lockup_account();
        testing_env_with_promise_results(
            context.clone(),
            PromiseResult::Successful(b"false".to_vec()),
        );
        contract.on_whitelist_is_whitelisted(false, staking_pool.clone());
    }

    #[test]
    #[should_panic(expected = "Staking pool is not selected")]
    fn test_staking_pool_unselecting_non_selected() {
        let (mut context, mut contract) = lockup_only_setup();
        context.predecessor_account_id = account_owner();
        context.signer_account_id = account_owner();
        context.signer_account_pk = public_key(2).try_into().unwrap();

        // Unselecting staking pool
        testing_env!(context.clone());
        contract.unselect_staking_pool();
    }

    #[test]
    #[should_panic(expected = "There is still a deposit on the staking pool")]
    fn test_staking_pool_unselecting_with_deposit() {
        let (mut context, mut contract) = lockup_only_setup();
        context.predecessor_account_id = account_owner();
        context.signer_account_id = account_owner();
        context.signer_account_pk = public_key(2).try_into().unwrap();

        // Selecting staking pool
        let staking_pool = "staking_pool".to_string();
        testing_env!(context.clone());
        contract.select_staking_pool(staking_pool.clone());

        context.predecessor_account_id = lockup_account();
        testing_env_with_promise_results(
            context.clone(),
            PromiseResult::Successful(b"true".to_vec()),
        );
        contract.on_whitelist_is_whitelisted(true, staking_pool.clone());

        // Deposit to the staking_pool
        let amount = to_yocto(LOCKUP_NEAR - 100);
        context.predecessor_account_id = account_owner();
        testing_env!(context.clone());
        contract.deposit_to_staking_pool(amount.into());
        context.account_balance = env::account_balance();

        context.predecessor_account_id = lockup_account();
        testing_env_with_promise_results(context.clone(), PromiseResult::Successful(vec![]));
        contract.on_staking_pool_deposit(amount.into());

        // Unselecting staking pool
        context.predecessor_account_id = account_owner();
        testing_env!(context.clone());
        contract.unselect_staking_pool();
    }

    #[test]
    fn test_staking_pool_owner_balance() {
        let (mut context, mut contract) = lockup_only_setup();
        context.predecessor_account_id = account_owner();
        context.signer_account_id = account_owner();
        context.signer_account_pk = public_key(2).try_into().unwrap();
        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR + 1);

        let lockup_amount = to_yocto(LOCKUP_NEAR);
        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, lockup_amount);
        context.is_view = false;

        // Selecting staking pool
        let staking_pool = "staking_pool".to_string();
        testing_env!(context.clone());
        contract.select_staking_pool(staking_pool.clone());

        context.predecessor_account_id = lockup_account();
        testing_env_with_promise_results(
            context.clone(),
            PromiseResult::Successful(b"true".to_vec()),
        );
        contract.on_whitelist_is_whitelisted(true, staking_pool.clone());

        // Deposit to the staking_pool
        let mut total_amount = 0;
        let amount = to_yocto(100);
        for _ in 1..=5 {
            total_amount += amount;
            context.predecessor_account_id = account_owner();
            testing_env!(context.clone());
            contract.deposit_to_staking_pool(amount.into());
            context.account_balance = env::account_balance();
            assert_eq!(context.account_balance, lockup_amount - total_amount);

            context.predecessor_account_id = lockup_account();
            testing_env_with_promise_results(context.clone(), PromiseResult::Successful(vec![]));
            contract.on_staking_pool_deposit(amount.into());
            context.is_view = true;
            testing_env!(context.clone());
            assert_eq!(contract.get_known_deposited_balance().0, total_amount);
            assert_eq!(contract.get_owners_balance().0, lockup_amount);
            assert_eq!(
                contract.get_liquid_owners_balance().0,
                lockup_amount - total_amount - MIN_BALANCE_FOR_STORAGE
            );
            context.is_view = false;
        }

        // Withdrawing from the staking_pool. Plus one extra time as a reward
        let mut total_withdrawn_amount = 0;
        for _ in 1..=6 {
            total_withdrawn_amount += amount;
            context.predecessor_account_id = account_owner();
            testing_env!(context.clone());
            contract.withdraw_from_staking_pool(amount.into());
            context.account_balance += amount;
            assert_eq!(
                context.account_balance,
                lockup_amount - total_amount + total_withdrawn_amount
            );

            context.predecessor_account_id = lockup_account();
            testing_env_with_promise_results(context.clone(), PromiseResult::Successful(vec![]));
            contract.on_staking_pool_withdraw(amount.into());
            context.is_view = true;
            testing_env!(context.clone());
            assert_eq!(
                contract.get_known_deposited_balance().0,
                total_amount.saturating_sub(total_withdrawn_amount)
            );
            assert_eq!(
                contract.get_owners_balance().0,
                lockup_amount + total_withdrawn_amount.saturating_sub(total_amount)
            );
            assert_eq!(
                contract.get_liquid_owners_balance().0,
                lockup_amount - total_amount + total_withdrawn_amount - MIN_BALANCE_FOR_STORAGE
            );
            context.is_view = false;
        }
    }

    #[test]
    #[should_panic(expected = "Foundation account can't be added without vesting schedule")]
    fn test_init_foundation_key_no_vesting() {
        let context = basic_context();
        testing_env!(context.clone());
        new_contract(true, None, None, true);
    }

    #[test]
    #[should_panic(expected = "Foundation account can't be added without vesting schedule")]
    fn test_init_foundation_key_no_vesting_with_release() {
        let context = basic_context();
        testing_env!(context.clone());
        new_contract(true, None, Some(to_nanos(YEAR).into()), true);
    }

    #[test]
    fn test_lock_timestmap() {
        let mut context = basic_context();
        testing_env!(context.clone());
        let contract = LockupContract::new(
            account_owner(),
            0.into(),
            Some(to_ts(GENESIS_TIME_IN_DAYS + YEAR).into()),
            TransfersInformation::TransfersDisabled {
                transfer_poll_account_id: AccountId::from("transfers"),
            },
            None,
            None,
            AccountId::from("whitelist"),
            None,
        );

        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, 0);
        assert_eq!(contract.get_liquid_owners_balance().0, 0);
        assert_eq!(contract.get_locked_vested_amount().0, to_yocto(1000));
        assert_eq!(contract.get_locked_amount().0, to_yocto(1000));
        assert_eq!(contract.get_unvested_amount().0, to_yocto(0));
        assert!(!contract.are_transfers_enabled());

        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR);
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, 0);
        assert_eq!(contract.get_liquid_owners_balance().0, 0);
        assert_eq!(contract.get_locked_vested_amount().0, to_yocto(1000));
        assert_eq!(contract.get_locked_amount().0, to_yocto(1000));
        assert_eq!(contract.get_unvested_amount().0, to_yocto(0));
    }

    #[test]
    fn test_lock_timestmap_transfer_enabled() {
        let mut context = basic_context();
        testing_env!(context.clone());
        let contract = LockupContract::new(
            account_owner(),
            0.into(),
            Some(to_ts(GENESIS_TIME_IN_DAYS + YEAR).into()),
            TransfersInformation::TransfersEnabled {
                transfers_timestamp: to_ts(GENESIS_TIME_IN_DAYS + YEAR / 2).into(),
            },
            None,
            None,
            AccountId::from("whitelist"),
            None,
        );

        context.is_view = true;
        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR);
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, to_yocto(1000));
        assert_eq!(
            contract.get_liquid_owners_balance().0,
            to_yocto(1000) - MIN_BALANCE_FOR_STORAGE
        );
        assert_eq!(contract.get_locked_vested_amount().0, to_yocto(0));
        assert_eq!(contract.get_locked_amount().0, to_yocto(0));
        assert_eq!(contract.get_unvested_amount().0, to_yocto(0));
    }

    #[test]
    fn test_termination_no_staking() {
        let mut context = basic_context();
        testing_env!(context.clone());
        let mut contract = new_contract(true, new_vesting_schedule(0), None, true);

        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, 0);
        assert_eq!(contract.get_liquid_owners_balance().0, 0);
        assert_eq!(contract.get_locked_vested_amount().0, to_yocto(250));
        assert_eq!(contract.get_locked_amount().0, to_yocto(1000));
        assert_eq!(contract.get_unvested_amount().0, to_yocto(750));

        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR);
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, to_yocto(500));
        assert_eq!(contract.get_liquid_owners_balance().0, to_yocto(500));
        assert_eq!(contract.get_locked_vested_amount().0, to_yocto(0));
        assert_eq!(contract.get_locked_amount().0, to_yocto(500));
        assert_eq!(contract.get_unvested_amount().0, to_yocto(500));

        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + 2 * YEAR);
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, to_yocto(750));
        assert_eq!(contract.get_liquid_owners_balance().0, to_yocto(750));
        assert_eq!(contract.get_locked_amount().0, to_yocto(250));
        assert_eq!(contract.get_unvested_amount().0, to_yocto(250));

        // Terminating
        context.is_view = false;
        context.predecessor_account_id = account_foundation();
        context.signer_account_pk = public_key(3).into();
        testing_env!(context.clone());
        contract.terminate_vesting();

        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, to_yocto(750));
        assert_eq!(contract.get_liquid_owners_balance().0, to_yocto(750));
        assert_eq!(contract.get_locked_amount().0, to_yocto(250));
        assert_eq!(contract.get_unvested_amount().0, to_yocto(250));
        assert_eq!(contract.get_terminated_unvested_balance().0, to_yocto(250));
        assert_eq!(
            contract.get_terminated_unvested_balance_deficit().0,
            to_yocto(0)
        );
        assert_eq!(
            contract.get_termination_status(),
            Some(TerminationStatus::ReadyToWithdraw)
        );

        // Withdrawing
        context.is_view = false;
        testing_env!(context.clone());
        let receiver_id = "near".to_string();
        contract.termination_withdraw(receiver_id.clone());
        context.account_balance = env::account_balance();

        context.predecessor_account_id = lockup_account();
        testing_env_with_promise_results(context.clone(), PromiseResult::Successful(vec![]));
        contract.on_withdraw_unvested_amount(to_yocto(250).into(), receiver_id);

        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, to_yocto(750));
        assert_eq!(
            contract.get_liquid_owners_balance().0,
            to_yocto(750) - MIN_BALANCE_FOR_STORAGE
        );
        assert_eq!(contract.get_unvested_amount().0, to_yocto(0));
        assert_eq!(contract.get_terminated_unvested_balance().0, to_yocto(0));
        assert_eq!(contract.get_termination_status(), None);
    }

    #[test]
    fn test_release_duration() {
        let mut context = basic_context();
        testing_env!(context.clone());
        let contract = new_contract(true, None, Some(to_nanos(4 * YEAR).into()), false);

        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, 0);
        assert_eq!(contract.get_liquid_owners_balance().0, 0);
        assert_eq!(contract.get_locked_vested_amount().0, to_yocto(1000));
        assert_eq!(contract.get_locked_amount().0, to_yocto(1000));
        assert_eq!(contract.get_unvested_amount().0, to_yocto(0));

        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR);
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, to_yocto(250));
        assert_eq!(contract.get_liquid_owners_balance().0, to_yocto(250));
        assert_eq!(contract.get_locked_vested_amount().0, to_yocto(750));
        assert_eq!(contract.get_locked_amount().0, to_yocto(750));

        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + 2 * YEAR);
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, to_yocto(500));
        assert_eq!(contract.get_liquid_owners_balance().0, to_yocto(500));
        assert_eq!(contract.get_locked_vested_amount().0, to_yocto(500));
        assert_eq!(contract.get_locked_amount().0, to_yocto(500));

        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + 3 * YEAR);
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, to_yocto(750));
        assert_eq!(contract.get_liquid_owners_balance().0, to_yocto(750));
        assert_eq!(contract.get_locked_amount().0, to_yocto(250));
    }

    #[test]
    fn test_vesting_and_release_duration() {
        let mut context = basic_context();
        testing_env!(context.clone());
        let contract = new_contract(
            true,
            new_vesting_schedule(0),
            Some(to_nanos(4 * YEAR).into()),
            true,
        );

        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, 0);
        assert_eq!(contract.get_liquid_owners_balance().0, 0);
        assert_eq!(contract.get_locked_vested_amount().0, to_yocto(250));
        assert_eq!(contract.get_locked_amount().0, to_yocto(1000));
        assert_eq!(contract.get_unvested_amount().0, to_yocto(750));

        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR);
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, to_yocto(250));
        assert_eq!(contract.get_liquid_owners_balance().0, to_yocto(250));
        assert_eq!(contract.get_locked_vested_amount().0, to_yocto(250));
        assert_eq!(contract.get_locked_amount().0, to_yocto(750));
        assert_eq!(contract.get_unvested_amount().0, to_yocto(500));

        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + 2 * YEAR);
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, to_yocto(500));
        assert_eq!(contract.get_liquid_owners_balance().0, to_yocto(500));
        assert_eq!(contract.get_locked_vested_amount().0, to_yocto(250));
        assert_eq!(contract.get_locked_amount().0, to_yocto(500));
        assert_eq!(contract.get_unvested_amount().0, to_yocto(250));

        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + 3 * YEAR);
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, to_yocto(750));
        assert_eq!(contract.get_liquid_owners_balance().0, to_yocto(750));
        assert_eq!(contract.get_locked_vested_amount().0, to_yocto(250));
        assert_eq!(contract.get_locked_amount().0, to_yocto(250));
        assert_eq!(contract.get_unvested_amount().0, to_yocto(0));

        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + 4 * YEAR);
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, to_yocto(1000));
        assert_eq!(
            contract.get_liquid_owners_balance().0,
            to_yocto(1000) - MIN_BALANCE_FOR_STORAGE
        );
        assert_eq!(contract.get_locked_vested_amount().0, to_yocto(0));
        assert_eq!(contract.get_locked_amount().0, to_yocto(0));
        assert_eq!(contract.get_unvested_amount().0, to_yocto(0));
    }

    #[test]
    fn test_vesting_post_transfers_and_release_duration() {
        let mut context = basic_context();
        testing_env!(context.clone());
        let contract = new_contract(
            true,
            new_vesting_schedule(YEAR * 2),
            Some(to_nanos(4 * YEAR).into()),
            true,
        );

        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, 0);
        assert_eq!(contract.get_liquid_owners_balance().0, 0);
        assert_eq!(contract.get_locked_vested_amount().0, to_yocto(0));
        assert_eq!(contract.get_locked_amount().0, to_yocto(1000));
        assert_eq!(contract.get_unvested_amount().0, to_yocto(1000));

        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR);
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, to_yocto(0));
        assert_eq!(contract.get_liquid_owners_balance().0, to_yocto(0));
        assert_eq!(contract.get_locked_vested_amount().0, to_yocto(0));
        assert_eq!(contract.get_locked_amount().0, to_yocto(1000));
        assert_eq!(contract.get_unvested_amount().0, to_yocto(1000));

        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + 2 * YEAR);
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, to_yocto(250));
        assert_eq!(contract.get_liquid_owners_balance().0, to_yocto(250));
        assert_eq!(contract.get_locked_vested_amount().0, to_yocto(0));
        assert_eq!(contract.get_locked_amount().0, to_yocto(750));
        assert_eq!(contract.get_unvested_amount().0, to_yocto(750));

        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + 3 * YEAR);
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, to_yocto(500));
        assert_eq!(contract.get_liquid_owners_balance().0, to_yocto(500));
        assert_eq!(contract.get_locked_vested_amount().0, to_yocto(0));
        assert_eq!(contract.get_locked_amount().0, to_yocto(500));
        assert_eq!(contract.get_unvested_amount().0, to_yocto(500));

        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + 4 * YEAR);
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, to_yocto(750));
        assert_eq!(contract.get_liquid_owners_balance().0, to_yocto(750));
        assert_eq!(contract.get_locked_vested_amount().0, to_yocto(0));
        assert_eq!(contract.get_locked_amount().0, to_yocto(250));
        assert_eq!(contract.get_unvested_amount().0, to_yocto(250));

        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + 5 * YEAR);
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, to_yocto(1000));
        assert_eq!(
            contract.get_liquid_owners_balance().0,
            to_yocto(1000) - MIN_BALANCE_FOR_STORAGE
        );
        assert_eq!(contract.get_locked_vested_amount().0, to_yocto(0));
        assert_eq!(contract.get_locked_amount().0, to_yocto(0));
        assert_eq!(contract.get_unvested_amount().0, to_yocto(0));
    }

    #[test]
    fn test_termination_no_staking_with_release_duration() {
        let mut context = basic_context();
        testing_env!(context.clone());
        let mut contract = new_contract(
            true,
            new_vesting_schedule(0),
            Some(to_nanos(4 * YEAR).into()),
            true,
        );

        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, 0);
        assert_eq!(contract.get_liquid_owners_balance().0, 0);
        assert_eq!(contract.get_locked_vested_amount().0, to_yocto(250));
        assert_eq!(contract.get_locked_amount().0, to_yocto(1000));
        assert_eq!(contract.get_unvested_amount().0, to_yocto(750));

        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + 2 * YEAR);
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, to_yocto(500));
        assert_eq!(contract.get_liquid_owners_balance().0, to_yocto(500));
        assert_eq!(contract.get_locked_vested_amount().0, to_yocto(250));
        assert_eq!(contract.get_locked_amount().0, to_yocto(500));
        assert_eq!(contract.get_unvested_amount().0, to_yocto(250));

        // Terminating
        context.is_view = false;
        context.predecessor_account_id = account_foundation();
        context.signer_account_pk = public_key(3).into();
        testing_env!(context.clone());
        contract.terminate_vesting();

        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, to_yocto(500));
        assert_eq!(contract.get_liquid_owners_balance().0, to_yocto(500));
        assert_eq!(contract.get_locked_vested_amount().0, to_yocto(250));
        assert_eq!(contract.get_locked_amount().0, to_yocto(500));
        assert_eq!(contract.get_unvested_amount().0, to_yocto(250));
        assert_eq!(contract.get_terminated_unvested_balance().0, to_yocto(250));
        assert_eq!(
            contract.get_terminated_unvested_balance_deficit().0,
            to_yocto(0)
        );
        assert_eq!(
            contract.get_termination_status(),
            Some(TerminationStatus::ReadyToWithdraw)
        );

        // Withdrawing
        context.is_view = false;
        testing_env!(context.clone());
        let receiver_id = "near".to_string();
        contract.termination_withdraw(receiver_id.clone());
        context.account_balance = env::account_balance();

        context.predecessor_account_id = lockup_account();
        testing_env_with_promise_results(context.clone(), PromiseResult::Successful(vec![]));
        contract.on_withdraw_unvested_amount(to_yocto(250).into(), receiver_id);

        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, to_yocto(500));
        assert_eq!(contract.get_liquid_owners_balance().0, to_yocto(500));
        assert_eq!(contract.get_locked_vested_amount().0, to_yocto(250));
        assert_eq!(contract.get_unvested_amount().0, to_yocto(0));
        assert_eq!(contract.get_terminated_unvested_balance().0, to_yocto(0));
        assert_eq!(contract.get_termination_status(), None);

        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + 3 * YEAR);
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, to_yocto(750));
        assert_eq!(
            contract.get_liquid_owners_balance().0,
            to_yocto(750) - MIN_BALANCE_FOR_STORAGE
        );
        assert_eq!(contract.get_locked_vested_amount().0, to_yocto(0));
    }

    #[test]
    fn test_termination_before_cliff() {
        let lockup_amount = to_yocto(1000);
        let mut context = basic_context();
        testing_env!(context.clone());
        let mut contract = new_contract(true, new_vesting_schedule(YEAR), None, true);

        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, 0);
        assert_eq!(contract.get_liquid_owners_balance().0, 0);
        assert_eq!(contract.get_locked_amount().0, lockup_amount);
        assert_eq!(contract.get_unvested_amount().0, lockup_amount);
        assert_eq!(contract.get_locked_vested_amount().0, 0);

        // Terminating
        context.is_view = false;
        context.predecessor_account_id = account_foundation();
        context.signer_account_pk = public_key(3).into();
        testing_env!(context.clone());
        contract.terminate_vesting();

        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, 0);
        assert_eq!(contract.get_liquid_owners_balance().0, 0);
        assert_eq!(contract.get_locked_amount().0, lockup_amount);
        assert_eq!(contract.get_unvested_amount().0, lockup_amount);
        assert_eq!(contract.get_locked_vested_amount().0, 0);
        assert_eq!(contract.get_terminated_unvested_balance().0, lockup_amount);
        assert_eq!(
            contract.get_terminated_unvested_balance_deficit().0,
            MIN_BALANCE_FOR_STORAGE
        );
        assert_eq!(
            contract.get_termination_status(),
            Some(TerminationStatus::ReadyToWithdraw)
        );

        // Withdrawing
        context.is_view = false;
        testing_env!(context.clone());
        let receiver_id = account_foundation();
        contract.termination_withdraw(receiver_id.clone());
        context.account_balance = env::account_balance();
        assert_eq!(context.account_balance, MIN_BALANCE_FOR_STORAGE);

        context.predecessor_account_id = lockup_account();
        testing_env_with_promise_results(context.clone(), PromiseResult::Successful(vec![]));
        contract.on_withdraw_unvested_amount(
            (lockup_amount - MIN_BALANCE_FOR_STORAGE).into(),
            receiver_id,
        );

        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_unvested_amount().0, MIN_BALANCE_FOR_STORAGE);
        assert_eq!(contract.get_owners_balance().0, 0);
        assert_eq!(contract.get_liquid_owners_balance().0, 0);
        assert_eq!(contract.get_locked_vested_amount().0, 0);
        assert_eq!(
            contract.get_terminated_unvested_balance().0,
            MIN_BALANCE_FOR_STORAGE
        );
        assert_eq!(
            contract.get_terminated_unvested_balance_deficit().0,
            MIN_BALANCE_FOR_STORAGE
        );
        assert_eq!(
            contract.get_termination_status(),
            Some(TerminationStatus::ReadyToWithdraw)
        );
    }

    #[test]
    fn test_termination_with_staking() {
        let lockup_amount = to_yocto(1000);
        let mut context = basic_context();
        testing_env!(context.clone());
        let mut contract = new_contract(true, new_vesting_schedule(0), None, true);

        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, to_yocto(0));
        assert_eq!(contract.get_liquid_owners_balance().0, to_yocto(0));
        assert_eq!(contract.get_locked_amount().0, lockup_amount);
        assert_eq!(contract.get_unvested_amount().0, to_yocto(750));
        assert_eq!(contract.get_locked_vested_amount().0, to_yocto(250));
        context.is_view = false;

        context.predecessor_account_id = account_owner();
        context.signer_account_pk = public_key(2).into();
        testing_env!(context.clone());

        // Selecting staking pool
        let staking_pool = "staking_pool".to_string();
        testing_env!(context.clone());
        contract.select_staking_pool(staking_pool.clone());

        context.predecessor_account_id = lockup_account();
        testing_env_with_promise_results(
            context.clone(),
            PromiseResult::Successful(b"true".to_vec()),
        );
        contract.on_whitelist_is_whitelisted(true, staking_pool.clone());

        // Deposit to the staking_pool
        let stake_amount = to_yocto(LOCKUP_NEAR - 100);
        context.predecessor_account_id = account_owner();
        testing_env!(context.clone());
        contract.deposit_to_staking_pool(stake_amount.into());
        context.account_balance = env::account_balance();

        context.predecessor_account_id = lockup_account();
        testing_env_with_promise_results(context.clone(), PromiseResult::Successful(vec![]));
        contract.on_staking_pool_deposit(stake_amount.into());

        // Staking on the staking pool
        context.predecessor_account_id = account_owner();
        testing_env!(context.clone());
        contract.stake(stake_amount.into());

        context.predecessor_account_id = lockup_account();
        testing_env_with_promise_results(context.clone(), PromiseResult::Successful(vec![]));
        contract.on_staking_pool_stake(stake_amount.into());

        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, to_yocto(0));
        assert_eq!(contract.get_liquid_owners_balance().0, to_yocto(0));
        assert_eq!(contract.get_known_deposited_balance().0, stake_amount);
        assert_eq!(contract.get_locked_amount().0, lockup_amount);
        assert_eq!(contract.get_locked_vested_amount().0, to_yocto(250));
        assert_eq!(contract.get_unvested_amount().0, to_yocto(750));
        context.is_view = false;

        // Foundation terminating
        context.is_view = false;
        context.predecessor_account_id = account_foundation();
        context.signer_account_pk = public_key(3).into();
        testing_env!(context.clone());
        contract.terminate_vesting();

        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, to_yocto(0));
        assert_eq!(contract.get_liquid_owners_balance().0, to_yocto(0));
        assert_eq!(contract.get_locked_amount().0, lockup_amount);
        assert_eq!(contract.get_unvested_amount().0, to_yocto(750));
        assert_eq!(contract.get_locked_vested_amount().0, to_yocto(250));
        assert_eq!(contract.get_terminated_unvested_balance().0, to_yocto(750));
        assert_eq!(
            contract.get_terminated_unvested_balance_deficit().0,
            to_yocto(650) + MIN_BALANCE_FOR_STORAGE
        );
        assert_eq!(
            contract.get_termination_status(),
            Some(TerminationStatus::VestingTerminatedWithDeficit)
        );

        // Proceeding with unstaking from the pool due to termination.
        context.is_view = false;
        testing_env!(context.clone());
        contract.termination_prepare_to_withdraw();
        assert_eq!(
            contract.get_termination_status(),
            Some(TerminationStatus::UnstakingInProgress)
        );

        let stake_amount_with_rewards = stake_amount + to_yocto(50);
        context.predecessor_account_id = lockup_account();
        testing_env_with_promise_results(
            context.clone(),
            PromiseResult::Successful(format!("{}", stake_amount_with_rewards).into_bytes()),
        );
        contract.on_get_account_staked_balance_to_unstake(stake_amount_with_rewards.into());

        testing_env_with_promise_results(context.clone(), PromiseResult::Successful(vec![]));
        contract.on_staking_pool_unstake_for_termination(stake_amount_with_rewards.into());

        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(
            contract.get_termination_status(),
            Some(TerminationStatus::EverythingUnstaked)
        );

        // Proceeding with withdrawing from the pool due to termination.
        context.is_view = false;
        context.predecessor_account_id = account_foundation();
        testing_env!(context.clone());
        contract.termination_prepare_to_withdraw();
        assert_eq!(
            contract.get_termination_status(),
            Some(TerminationStatus::WithdrawingFromStakingPoolInProgress)
        );

        let withdraw_amount_with_extra_rewards = stake_amount_with_rewards + to_yocto(1);
        context.predecessor_account_id = lockup_account();
        testing_env_with_promise_results(
            context.clone(),
            PromiseResult::Successful(
                format!("{}", withdraw_amount_with_extra_rewards).into_bytes(),
            ),
        );
        contract
            .on_get_account_unstaked_balance_to_withdraw(withdraw_amount_with_extra_rewards.into());
        context.account_balance += withdraw_amount_with_extra_rewards;

        testing_env_with_promise_results(context.clone(), PromiseResult::Successful(vec![]));
        contract
            .on_staking_pool_withdraw_for_termination(withdraw_amount_with_extra_rewards.into());

        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, to_yocto(51));
        assert_eq!(contract.get_liquid_owners_balance().0, to_yocto(51));
        assert_eq!(contract.get_locked_amount().0, lockup_amount);
        assert_eq!(contract.get_unvested_amount().0, to_yocto(750));
        assert_eq!(contract.get_terminated_unvested_balance().0, to_yocto(750));
        assert_eq!(contract.get_locked_vested_amount().0, to_yocto(250));
        assert_eq!(contract.get_terminated_unvested_balance_deficit().0, 0);
        assert_eq!(contract.get_known_deposited_balance().0, 0);
        assert_eq!(
            contract.get_termination_status(),
            Some(TerminationStatus::ReadyToWithdraw)
        );

        // Withdrawing
        context.is_view = false;
        context.predecessor_account_id = account_foundation();
        testing_env!(context.clone());
        let receiver_id = account_foundation();
        contract.termination_withdraw(receiver_id.clone());
        context.account_balance = env::account_balance();
        assert_eq!(context.account_balance, to_yocto(250 + 51));

        context.predecessor_account_id = lockup_account();
        testing_env_with_promise_results(context.clone(), PromiseResult::Successful(vec![]));
        contract.on_withdraw_unvested_amount(to_yocto(750).into(), receiver_id);

        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, to_yocto(51));
        assert_eq!(contract.get_liquid_owners_balance().0, to_yocto(51));
        assert_eq!(contract.get_locked_amount().0, to_yocto(250));
        assert_eq!(contract.get_locked_vested_amount().0, to_yocto(250));
        assert_eq!(contract.get_unvested_amount().0, 0);
        assert_eq!(contract.get_terminated_unvested_balance().0, 0);
        assert_eq!(contract.get_terminated_unvested_balance_deficit().0, 0);
        assert_eq!(contract.get_termination_status(), None);

        // Checking the balance becomes unlocked later
        context.block_timestamp = to_ts(GENESIS_TIME_IN_DAYS + YEAR + 1);
        testing_env!(context.clone());
        assert_eq!(contract.get_owners_balance().0, to_yocto(301));
        assert_eq!(
            contract.get_liquid_owners_balance().0,
            to_yocto(301) - MIN_BALANCE_FOR_STORAGE
        );
        assert_eq!(contract.get_locked_vested_amount().0, 0);
        assert_eq!(contract.get_locked_amount().0, 0);
    }
}
