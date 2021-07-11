mod types;
mod utils;

pub use crate::types::*;
use crate::utils::*;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::json_types::{ValidAccountId, U128};
use near_sdk::serde::Serialize;
use near_sdk::{env, ext_contract, near_bindgen, AccountId, Balance, Promise};

/// There is no deposit balance attached.
const NO_DEPOSIT: Balance = 0;
const TRANSFERS_STARTED: u64 = 1602614338293769340; /* 13 October 2020 18:38:58.293 */

#[global_allocator]
static ALLOC: near_sdk::wee_alloc::WeeAlloc<'_> = near_sdk::wee_alloc::WeeAlloc::INIT;

const CODE: &[u8] = include_bytes!("../../lockup/res/lockup_contract.wasm");

pub mod gas {
    use near_sdk::Gas;

    /// The base amount of gas for a regular execution.
    const BASE: Gas = 25_000_000_000_000;

    /// The amount of Gas the contract will attach to the promise to create the lockup.
    pub const LOCKUP_NEW: Gas = BASE;

    /// The amount of Gas the contract will attach to the callback to itself.
    /// The base for the execution and the base for cash rollback.
    pub const CALLBACK: Gas = BASE;
}

const MIN_ATTACHED_BALANCE: Balance = 3_500_000_000_000_000_000_000_000;

/// External interface for the callbacks to self.
#[ext_contract(ext_self)]
pub trait ExtSelf {
    fn on_lockup_create(
        &mut self,
        lockup_account_id: AccountId,
        attached_deposit: U128,
        predecessor_account_id: AccountId,
    ) -> bool;
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct LockupFactory {
    whitelist_account_id: AccountId,
    foundation_account_id: AccountId,
}

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct LockupArgs {
    owner_account_id: ValidAccountId,
    lockup_duration: WrappedDuration,
    lockup_timestamp: Option<WrappedTimestamp>,
    transfers_information: TransfersInformation,
    vesting_schedule: Option<VestingScheduleOrHash>,
    release_duration: Option<WrappedDuration>,
    staking_pool_whitelist_account_id: AccountId,
    foundation_account_id: Option<AccountId>,
}

impl Default for LockupFactory {
    fn default() -> Self {
        env::panic(b"LockupFactory should be initialized before usage")
    }
}

#[near_bindgen]
impl LockupFactory {
    #[init]
    pub fn new(
        whitelist_account_id: ValidAccountId,
        foundation_account_id: ValidAccountId,
    ) -> Self {
        assert!(!env::state_exists(), "The contract is already initialized");
        assert!(
            env::current_account_id().len() <= 23,
            "The account ID of this contract can't be more than 23 characters"
        );

        Self {
            whitelist_account_id: whitelist_account_id.into(),
            foundation_account_id: foundation_account_id.into(),
        }
    }

    /// Returns the foundation account id.
    pub fn get_foundation_account_id(&self) -> AccountId {
        self.foundation_account_id.clone()
    }

    /// Returns the lockup master account id.
    pub fn get_lockup_master_account_id(&self) -> AccountId {
        env::current_account_id()
    }

    /// Returns minimum attached balance.
    pub fn get_min_attached_balance(&self) -> U128 {
        MIN_ATTACHED_BALANCE.into()
    }

    #[payable]
    pub fn create(
        &mut self,
        owner_account_id: ValidAccountId,
        lockup_duration: WrappedDuration,
        lockup_timestamp: Option<WrappedTimestamp>,
        vesting_schedule: Option<VestingScheduleOrHash>,
        release_duration: Option<WrappedDuration>,
    ) -> Promise {
        assert!(env::attached_deposit() >= MIN_ATTACHED_BALANCE, "Not enough attached deposit");

        let byte_slice = env::sha256(owner_account_id.as_ref().as_bytes());
        let lockup_account_id =
            format!("{}.{}", hex::encode(&byte_slice[..20]), env::current_account_id());

        let mut foundation_account: Option<AccountId> = None;
        if vesting_schedule.is_some() {
            foundation_account = Some(self.foundation_account_id.clone());
        };

        let transfers_enabled: WrappedTimestamp = TRANSFERS_STARTED.into();
        Promise::new(lockup_account_id.clone())
            .create_account()
            .deploy_contract(CODE.to_vec())
            .transfer(env::attached_deposit())
            .function_call(
                b"new".to_vec(),
                near_sdk::serde_json::to_vec(&LockupArgs {
                    owner_account_id,
                    lockup_duration,
                    lockup_timestamp,
                    transfers_information: TransfersInformation::TransfersEnabled {
                        transfers_timestamp: transfers_enabled,
                    },
                    vesting_schedule,
                    release_duration,
                    staking_pool_whitelist_account_id: self.whitelist_account_id.clone(),
                    foundation_account_id: foundation_account,
                })
                    .unwrap(),
                NO_DEPOSIT,
                gas::LOCKUP_NEW,
            )
            .then(ext_self::on_lockup_create(
                lockup_account_id,
                env::attached_deposit().into(),
                env::predecessor_account_id(),
                &env::current_account_id(),
                NO_DEPOSIT,
                gas::CALLBACK,
            ))
    }

    /// Callback after a lockup was created.
    /// Returns the promise if the lockup creation succeeded.
    /// Otherwise refunds the attached deposit and returns `false`.
    pub fn on_lockup_create(
        &mut self,
        lockup_account_id: AccountId,
        attached_deposit: U128,
        predecessor_account_id: AccountId,
    ) -> bool {
        assert_self();

        let lockup_account_created = is_promise_success();

        if lockup_account_created {
            env::log(
                format!("The lockup contract {} was successfully created.", lockup_account_id)
                    .as_bytes(),
            );
            true
        } else {
            env::log(
                format!(
                    "The lockup {} creation has failed. Returning attached deposit of {} to {}",
                    lockup_account_id, attached_deposit.0, predecessor_account_id
                )
                    .as_bytes(),
            );
            Promise::new(predecessor_account_id).transfer(attached_deposit.0);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    mod test_utils;

    use super::*;
    use near_sdk::{testing_env, MockedBlockchain, PromiseResult};
    use test_utils::*;

    fn new_vesting_schedule(offset_in_days: u64) -> VestingSchedule {
        VestingSchedule {
            start_timestamp: to_ts(GENESIS_TIME_IN_DAYS - YEAR + offset_in_days).into(),
            cliff_timestamp: to_ts(GENESIS_TIME_IN_DAYS + offset_in_days).into(),
            end_timestamp: to_ts(GENESIS_TIME_IN_DAYS + YEAR * 3 + offset_in_days).into(),
        }
    }

    #[test]
    fn test_get_factory_vars() {
        let mut context = VMContextBuilder::new()
            .current_account_id(account_factory())
            .predecessor_account_id(account_near())
            .finish();
        testing_env!(context.clone());

        let contract = LockupFactory::new(
            whitelist_account_id(),
            foundation_account_id(),
        );

        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_min_attached_balance().0, MIN_ATTACHED_BALANCE);
        assert_eq!(
            contract.get_foundation_account_id(),
            foundation_account_id().as_ref().to_string()
        );
        println!("{}", contract.get_lockup_master_account_id());
        assert_eq!(
            contract.get_lockup_master_account_id(),
            lockup_master_account_id().as_ref().to_string()
        );
    }

    #[test]
    fn test_create_lockup_success() {
        let mut context = VMContextBuilder::new()
            .current_account_id(account_factory())
            .predecessor_account_id(account_near())
            .finish();
        testing_env!(context.clone());

        let mut contract = LockupFactory::new(
            whitelist_account_id(),
            foundation_account_id(),
        );

        const LOCKUP_DURATION: u64 = 63036000000000000; /* 24 months */
        let lockup_duration: WrappedTimestamp = LOCKUP_DURATION.into();

        context.is_view = false;
        context.predecessor_account_id = String::from(account_tokens_owner());
        context.attached_deposit = ntoy(35);
        testing_env!(context.clone());
        contract.create(account_tokens_owner(), lockup_duration, None, None, None);

        context.predecessor_account_id = account_factory();
        context.attached_deposit = ntoy(0);
        testing_env_with_promise_results(context.clone(), PromiseResult::Successful(vec![]));
        println!("{}", lockup_account());
        contract.on_lockup_create(
            lockup_account(),
            ntoy(30).into(),
            String::from(account_tokens_owner()),
        );
    }

    #[test]
    fn test_create_lockup_with_vesting_success() {
        let mut context = VMContextBuilder::new()
            .current_account_id(account_factory())
            .predecessor_account_id(account_near())
            .finish();
        testing_env!(context.clone());

        let mut contract = LockupFactory::new(
            whitelist_account_id(),
            foundation_account_id(),
        );

        const LOCKUP_DURATION: u64 = 63036000000000000; /* 24 months */
        const LOCKUP_TIMESTAMP: u64 = 1661990400000000000; /* 1 September 2022 00:00:00 */
        let lockup_duration: WrappedTimestamp = LOCKUP_DURATION.into();
        let lockup_timestamp: WrappedTimestamp = LOCKUP_TIMESTAMP.into();

        let vesting_schedule = Some(new_vesting_schedule(10));

        let vesting_schedule = vesting_schedule.map(|vesting_schedule| {
            VestingScheduleOrHash::VestingHash(
                VestingScheduleWithSalt { vesting_schedule, salt: SALT.to_vec().into() }
                    .hash()
                    .into(),
            )
        });

        context.is_view = false;
        context.predecessor_account_id = String::from(account_tokens_owner());
        context.attached_deposit = ntoy(35);
        testing_env!(context.clone());
        contract.create(
            account_tokens_owner(),
            lockup_duration,
            Some(lockup_timestamp),
            vesting_schedule,
            None,
        );

        context.predecessor_account_id = account_factory();
        context.attached_deposit = ntoy(0);
        testing_env_with_promise_results(context.clone(), PromiseResult::Successful(vec![]));
        contract.on_lockup_create(
            lockup_account(),
            ntoy(30).into(),
            String::from(account_tokens_owner()),
        );
    }

    #[test]
    #[should_panic(expected = "Not enough attached deposit")]
    fn test_create_lockup_not_enough_deposit() {
        let mut context = VMContextBuilder::new()
            .current_account_id(account_factory())
            .predecessor_account_id(account_near())
            .finish();
        testing_env!(context.clone());

        let mut contract = LockupFactory::new(
            whitelist_account_id(),
            foundation_account_id(),
        );

        const LOCKUP_DURATION: u64 = 63036000000000000; /* 24 months */
        let lockup_duration: WrappedTimestamp = LOCKUP_DURATION.into();

        context.is_view = false;
        context.predecessor_account_id = String::from(account_tokens_owner());
        context.attached_deposit = ntoy(1); /* Storage reduced to 3.5 NEAR */
        testing_env!(context.clone());
        contract.create(account_tokens_owner(), lockup_duration, None, None, None);
    }

    #[test]
    fn test_create_lockup_rollback() {
        let mut context = VMContextBuilder::new()
            .current_account_id(account_factory())
            .predecessor_account_id(account_near())
            .finish();
        testing_env!(context.clone());

        let mut contract = LockupFactory::new(
            whitelist_account_id(),
            foundation_account_id(),
        );

        const LOCKUP_DURATION: u64 = 63036000000000000; /* 24 months */
        let lockup_duration: WrappedTimestamp = LOCKUP_DURATION.into();

        context.is_view = false;
        context.predecessor_account_id = String::from(account_tokens_owner());
        context.attached_deposit = ntoy(35);
        testing_env!(context.clone());
        contract.create(account_tokens_owner(), lockup_duration, None, None, None);

        context.predecessor_account_id = account_factory();
        context.attached_deposit = ntoy(0);
        context.account_balance += ntoy(35);
        testing_env_with_promise_results(context.clone(), PromiseResult::Failed);
        let res = contract.on_lockup_create(
            lockup_account(),
            ntoy(35).into(),
            String::from(account_tokens_owner()),
        );

        match res {
            true => panic!("Unexpected result, should return false"),
            false => assert!(true),
        };
    }
}
