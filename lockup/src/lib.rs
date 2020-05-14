//! A smart contract that allows tokens lockup.

use borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::json_types::Base58PublicKey;
use near_sdk::{env, ext_contract, near_bindgen, AccountId, Promise};

pub mod types;
pub use crate::types::*;

pub mod utils;
pub use crate::utils::*;

pub mod callbacks;
pub use crate::callbacks::*;

pub mod foundation;
pub use crate::foundation::*;

pub mod gas;

pub mod getters;
pub use crate::getters::*;

pub mod internal;
pub use crate::internal::*;

pub mod owner;
pub use crate::owner::*;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

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

#[ext_contract(ext_staking_pool)]
pub trait ExtStakingPool {
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
            assert!(
                lockup_information.lockup_timestamp.is_none(),
                "Lockup timestamp should not be given when transfer voting information is present"
            );
        } else {
            assert!(
                lockup_information.lockup_timestamp.is_some(),
                "Lockup timestamp should be given when transfer voting information is absent"
            );
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

    pub fn to_nanos(num_days: u64) -> u64 {
        num_days * 86400_000_000_000
    }

    pub fn to_ts(num_days: u64) -> u64 {
        // 2018-08-01 UTC in nanoseconds
        1533081600_000_000_000 + to_nanos(num_days)
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
                lockup_timestamp: Some(to_ts(GENESIS_TIME_IN_DAYS).into()),
                lockup_duration: to_nanos(YEAR).into(),
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
