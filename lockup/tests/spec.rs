use lockup_contract::{
    LockupContractContract, TerminationStatus, TransfersInformation, VestingSchedule,
    VestingScheduleOrHash, VestingScheduleWithSalt, WrappedBalance, MIN_BALANCE_FOR_STORAGE
};
use near_sdk::borsh::BorshSerialize;
use near_sdk::json_types::{Base58PublicKey, U128};
use near_sdk::serde_json::json;
use near_sdk::{AccountId, Balance};
use near_sdk_sim::runtime::GenesisConfig;
use near_sdk_sim::{deploy, init_simulator, to_yocto, UserAccount};
use quickcheck_macros::quickcheck;
use std::convert::TryInto;

pub const MAX_GAS: u64 = 300000000000000;
pub const NO_DEPOSIT: Balance = 0;
pub const LOCKUP_ACCOUNT_ID: &str = "lockup";

const STAKING_POOL_WHITELIST_ACCOUNT_ID: &str = "staking-pool-whitelist";
const STAKING_POOL_ACCOUNT_ID: &str = "staking-pool";
const TRANSFER_POLL_ACCOUNT_ID: &str = "transfer-poll";

pub fn public_key(byte_val: u8) -> Base58PublicKey {
    let mut pk = vec![byte_val; 33];
    pk[0] = 0;
    Base58PublicKey(pk)
}

pub fn assert_almost_eq_with_max_delta(left: u128, right: u128, max_delta: u128) {
    assert!(
        std::cmp::max(left, right) - std::cmp::min(left, right) <= max_delta,
        "{}",
        format!(
            "Left {} is not even close to Right {} within delta {}",
            left, right, max_delta
        )
    );
}

pub fn assert_eq_with_gas(left: u128, right: u128) {
    assert_almost_eq_with_max_delta(left, right, to_yocto("0.005"));
}

pub fn assert_yocto_eq(left: u128, right: u128) {
    assert_almost_eq_with_max_delta(left, right, 1);
}

near_sdk_sim::lazy_static_include::lazy_static_include_bytes! {
    LOCKUP_WASM_BYTES => "res/lockup_contract.wasm",
    STAKING_POOL_WASM_BYTES => "../staking-pool/res/staking_pool.wasm",
    FAKE_VOTING_WASM_BYTES => "tests/res/fake_voting.wasm",
    WHITELIST_WASM_BYTES => "../whitelist/res/whitelist.wasm",
}

#[quickcheck]
fn lockup(lockup_amount: Balance, lockup_duration: u64, lockup_timestamp: u64) {
    let (root, _foundation, owner, _staking_pool) = basic_setup();

    let lockup = deploy!(
        contract: LockupContractContract,
        contract_id: LOCKUP_ACCOUNT_ID.to_string(),
        bytes: &LOCKUP_WASM_BYTES,
        signer_account: root,
        deposit: MIN_BALANCE_FOR_STORAGE + lockup_amount,
        gas: MAX_GAS,
        init_method: new(
            owner.account_id.clone(),
            lockup_duration.into(),
            None,
            TransfersInformation::TransfersEnabled {
                transfers_timestamp: lockup_timestamp.saturating_add(1).into(),
            },
            None,
            None,
            STAKING_POOL_WHITELIST_ACCOUNT_ID.to_string(),
            None
        )
    );

    root.borrow_runtime_mut().cur_block.block_timestamp = lockup_timestamp
        .saturating_add(lockup_duration)
        .saturating_sub(1);

    let locked_amount: U128 = owner
        .view_method_call(lockup.contract.get_locked_amount())
        .unwrap_json();
    assert_eq!(locked_amount.0, MIN_BALANCE_FOR_STORAGE + lockup_amount);

    let block_timestamp = root.borrow_runtime().cur_block.block_timestamp;
    root.borrow_runtime_mut().cur_block.block_timestamp = block_timestamp.saturating_add(2);

    let locked_amount: U128 = owner
        .view_method_call(lockup.contract.get_locked_amount())
        .unwrap_json();
    assert_eq!(locked_amount.0, 0);
}

#[test]
fn staking() {
    let lockup_amount = to_yocto("1000");
    let (root, foundation, owner, staking_pool) = basic_setup();

    let lockup = deploy!(
        contract: LockupContractContract,
        contract_id: LOCKUP_ACCOUNT_ID.to_string(),
        bytes: &LOCKUP_WASM_BYTES,
        signer_account: root,
        deposit: MIN_BALANCE_FOR_STORAGE + lockup_amount,
        gas: MAX_GAS,
        init_method: new(
            owner.account_id.clone(),
            1000000000.into(),
            None,
            TransfersInformation::TransfersDisabled {
                transfer_poll_account_id: "transfer-poll".to_string(),
            },
            None,
            None,
            STAKING_POOL_WHITELIST_ACCOUNT_ID.to_string(),
            None
        )
    );

    let owner_staking_account = &owner;

    let res: Option<AccountId> = owner
        .view_method_call(lockup.contract.get_staking_pool_account_id())
        .unwrap_json();
    assert_eq!(res, None);

    // Selecting staking pool
    owner_staking_account
        .function_call(
            lockup
                .contract
                .select_staking_pool(STAKING_POOL_ACCOUNT_ID.to_string()),
            MAX_GAS,
            0,
        )
        .assert_success();

    let res: Option<AccountId> = owner
        .view_method_call(lockup.contract.get_staking_pool_account_id())
        .unwrap_json();
    assert_eq!(res, Some(STAKING_POOL_ACCOUNT_ID.to_string()));
    let res: U128 = owner
        .view_method_call(lockup.contract.get_known_deposited_balance())
        .unwrap_json();
    assert_eq!(res.0, 0);

    // Depositing to the staking pool
    let staking_amount = lockup_amount - to_yocto("100");
    owner_staking_account
        .function_call(
            lockup
                .contract
                .deposit_to_staking_pool(U128(staking_amount)),
            MAX_GAS,
            0,
        )
        .assert_success();

    let res: U128 = owner
        .view_method_call(lockup.contract.get_known_deposited_balance())
        .unwrap_json();
    assert_eq!(res.0, staking_amount);

    // Staking on the staking pool
    owner_staking_account
        .function_call(lockup.contract.stake(U128(staking_amount)), MAX_GAS, 0)
        .assert_success();

    let res: U128 = owner
        .view(
            STAKING_POOL_ACCOUNT_ID.to_string(),
            "get_account_staked_balance",
            &json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })
                .to_string()
                .into_bytes(),
        )
        .unwrap_json();
    assert_yocto_eq(res.0, staking_amount);

    // Refreshing staking balance. Should be NOOP
    owner_staking_account
        .function_call(lockup.contract.refresh_staking_pool_balance(), MAX_GAS, 0)
        .assert_success();

    let res: U128 = owner
        .view_method_call(lockup.contract.get_known_deposited_balance())
        .unwrap_json();
    assert_yocto_eq(res.0, staking_amount);

    // Simulating rewards
    foundation
        .transfer(STAKING_POOL_ACCOUNT_ID.to_string(), to_yocto("10"))
        .assert_success();

    // Pinging the staking pool
    foundation
        .call(STAKING_POOL_ACCOUNT_ID.to_string(), "ping", b"", MAX_GAS, 0)
        .assert_success();

    let res: U128 = owner
        .view(
            STAKING_POOL_ACCOUNT_ID.to_string(),
            "get_account_staked_balance",
            &json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })
                .to_string()
                .into_bytes(),
        )
        .unwrap_json();
    let new_stake_amount = res.0;
    assert!(new_stake_amount > staking_amount);

    // Refresh staking balance again
    owner_staking_account
        .function_call(lockup.contract.refresh_staking_pool_balance(), MAX_GAS, 0)
        .assert_success();

    let res: U128 = owner
        .view_method_call(lockup.contract.get_known_deposited_balance())
        .unwrap_json();
    let new_total_balance = res.0;
    assert!(new_total_balance >= new_stake_amount);

    let res: U128 = owner
        .view_method_call(lockup.contract.get_owners_balance())
        .unwrap_json();
    // Account for gas rewards
    assert_eq_with_gas(res.0, new_total_balance - staking_amount);

    // Unstaking everything
    let res: bool = owner_staking_account
        .function_call(lockup.contract.unstake(U128(new_stake_amount)), MAX_GAS, 0)
        .unwrap_json();
    assert!(res);

    let res: U128 = owner
        .view(
            STAKING_POOL_ACCOUNT_ID.to_string(),
            "get_account_staked_balance",
            &json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })
                .to_string()
                .into_bytes(),
        )
        .unwrap_json();
    assert_eq_with_gas(res.0, 0);

    let res: U128 = owner
        .view(
            STAKING_POOL_ACCOUNT_ID.to_string(),
            "get_account_unstaked_balance",
            &json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })
                .to_string()
                .into_bytes(),
        )
        .unwrap_json();
    assert!(res.0 >= new_total_balance);

    root.borrow_runtime_mut().cur_block.block_height += 40;
    root.borrow_runtime_mut().cur_block.epoch_height += 4;

    // The standalone runtime doesn't unlock locked balance. Need to manually intervene.
    let mut pool = staking_pool.account().unwrap();
    pool.amount += pool.locked;
    pool.locked = 0;
    staking_pool
        .borrow_runtime_mut()
        .force_account_update(STAKING_POOL_ACCOUNT_ID.to_string(), &pool);

    // Withdrawing everything from the staking pool
    let res: bool = owner_staking_account
        .function_call(
            lockup
                .contract
                .withdraw_from_staking_pool(U128(new_total_balance)),
            MAX_GAS,
            0,
        )
        .unwrap_json();
    assert!(res);

    let res: U128 = owner
        .view_method_call(lockup.contract.get_known_deposited_balance())
        .unwrap_json();
    assert_eq!(res.0, 0);

    let res: U128 = owner
        .view_method_call(lockup.contract.get_owners_balance())
        .unwrap_json();
    assert_eq_with_gas(res.0, new_stake_amount - staking_amount);

    // Unselecting the staking pool
    owner_staking_account
        .function_call(lockup.contract.unselect_staking_pool(), MAX_GAS, 0)
        .assert_success();

    let res: Option<AccountId> = owner
        .view_method_call(lockup.contract.get_staking_pool_account_id())
        .unwrap_json();
    assert_eq!(res, None);
}

#[test]
fn staking_with_helpers() {
    let lockup_amount = to_yocto("1000");
    let (root, foundation, owner, staking_pool) = basic_setup();

    let lockup = deploy!(
        contract: LockupContractContract,
        contract_id: LOCKUP_ACCOUNT_ID.to_string(),
        bytes: &LOCKUP_WASM_BYTES,
        signer_account: root,
        deposit: MIN_BALANCE_FOR_STORAGE + lockup_amount,
        gas: MAX_GAS,
        init_method: new(
            owner.account_id.clone(),
            1000000000.into(),
            None,
            TransfersInformation::TransfersDisabled {
                transfer_poll_account_id: "transfer-poll".to_string(),
            },
            None,
            None,
            STAKING_POOL_WHITELIST_ACCOUNT_ID.to_string(),
            None
        )
    );

    let owner_staking_account = &owner;

    let res: Option<AccountId> = owner
        .view_method_call(lockup.contract.get_staking_pool_account_id())
        .unwrap_json();
    assert_eq!(res, None);

    // Selecting staking pool
    owner_staking_account
        .function_call(
            lockup
                .contract
                .select_staking_pool(STAKING_POOL_ACCOUNT_ID.to_string()),
            MAX_GAS,
            0,
        )
        .assert_success();

    let res: Option<AccountId> = owner
        .view_method_call(lockup.contract.get_staking_pool_account_id())
        .unwrap_json();
    assert_eq!(res, Some(STAKING_POOL_ACCOUNT_ID.to_string()));
    let res: U128 = owner
        .view_method_call(lockup.contract.get_known_deposited_balance())
        .unwrap_json();
    assert_eq!(res.0, 0);

    // Depositing and staking on the staking pool
    let staking_amount = lockup_amount - to_yocto("100");
    owner_staking_account
        .function_call(
            lockup.contract.deposit_and_stake(U128(staking_amount)),
            MAX_GAS,
            0,
        )
        .assert_success();

    let res: U128 = owner
        .view_method_call(lockup.contract.get_known_deposited_balance())
        .unwrap_json();
    assert_eq!(res.0, staking_amount);

    let res: U128 = owner
        .view(
            STAKING_POOL_ACCOUNT_ID.to_string(),
            "get_account_staked_balance",
            &json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })
                .to_string()
                .into_bytes(),
        )
        .unwrap_json();
    assert_yocto_eq(res.0, staking_amount);

    // Refreshing staking balance. Should be NOOP
    owner_staking_account
        .function_call(lockup.contract.refresh_staking_pool_balance(), MAX_GAS, 0)
        .assert_success();

    let res: U128 = owner
        .view_method_call(lockup.contract.get_known_deposited_balance())
        .unwrap_json();
    assert_yocto_eq(res.0, staking_amount);

    // Simulating rewards
    foundation
        .transfer(STAKING_POOL_ACCOUNT_ID.to_string(), to_yocto("10"))
        .assert_success();

    // Pinging the staking pool
    foundation
        .call(STAKING_POOL_ACCOUNT_ID.to_string(), "ping", b"", MAX_GAS, 0)
        .assert_success();

    let res: U128 = owner
        .view(
            STAKING_POOL_ACCOUNT_ID.to_string(),
            "get_account_staked_balance",
            &json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })
                .to_string()
                .into_bytes(),
        )
        .unwrap_json();
    let new_stake_amount = res.0;
    assert!(new_stake_amount > staking_amount);

    // Refresh staking balance again
    owner_staking_account
        .function_call(lockup.contract.refresh_staking_pool_balance(), MAX_GAS, 0)
        .assert_success();

    let res: U128 = owner
        .view_method_call(lockup.contract.get_known_deposited_balance())
        .unwrap_json();
    let new_total_balance = res.0;
    assert!(new_total_balance >= new_stake_amount);

    let res: U128 = owner
        .view_method_call(lockup.contract.get_owners_balance())
        .unwrap_json();
    // Account for gas rewards
    assert_eq_with_gas(res.0, new_total_balance - staking_amount);

    // Unstaking everything
    let res: bool = owner_staking_account
        .function_call(lockup.contract.unstake_all(), MAX_GAS, 0)
        .unwrap_json();
    assert!(res);

    let res: U128 = owner
        .view(
            STAKING_POOL_ACCOUNT_ID.to_string(),
            "get_account_staked_balance",
            &json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })
                .to_string()
                .into_bytes(),
        )
        .unwrap_json();
    assert_eq_with_gas(res.0, 0);

    let res: U128 = owner
        .view(
            STAKING_POOL_ACCOUNT_ID.to_string(),
            "get_account_unstaked_balance",
            &json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })
                .to_string()
                .into_bytes(),
        )
        .unwrap_json();
    let new_unstaked_amount = res.0;
    assert!(new_unstaked_amount >= new_total_balance);

    root.borrow_runtime_mut().cur_block.block_height += 40;
    root.borrow_runtime_mut().cur_block.epoch_height += 4;

    // The standalone runtime doesn't unlock locked balance. Need to manually intervene.
    let mut pool = staking_pool.account().unwrap();
    pool.amount += pool.locked;
    pool.locked = 0;
    staking_pool
        .borrow_runtime_mut()
        .force_account_update(STAKING_POOL_ACCOUNT_ID.to_string(), &pool);

    // Withdrawing everything from the staking pool
    let res: bool = owner_staking_account
        .function_call(lockup.contract.withdraw_all_from_staking_pool(), MAX_GAS, 0)
        .unwrap_json();
    assert!(res);

    let res: U128 = owner
        .view_method_call(lockup.contract.get_known_deposited_balance())
        .unwrap_json();
    assert_eq!(res.0, 0);

    let res: U128 = owner
        .view_method_call(lockup.contract.get_owners_balance())
        .unwrap_json();
    assert_eq_with_gas(res.0, new_stake_amount - staking_amount);

    // Unselecting the staking pool
    owner_staking_account
        .function_call(lockup.contract.unselect_staking_pool(), MAX_GAS, 0)
        .assert_success();

    let res: Option<AccountId> = owner
        .view_method_call(lockup.contract.get_staking_pool_account_id())
        .unwrap_json();
    assert_eq!(res, None);
}

#[test]
fn termination_with_staking_hashed() {
    let lockup_amount = to_yocto("1000");
    let (root, foundation, owner, staking_pool) = basic_setup();

    let start_timestamp = root.borrow_runtime().cur_block.block_timestamp;

    let vesting_schedule = VestingSchedule {
        start_timestamp: start_timestamp.into(),
        cliff_timestamp: (start_timestamp + 1000).into(),
        end_timestamp: (start_timestamp + 4000).into(),
    };
    let salt: Vec<u8> = [vec![1, 2, 3], b"VERY_LONG_SALT".to_vec()].concat();

    let lockup = deploy!(
        contract: LockupContractContract,
        contract_id: LOCKUP_ACCOUNT_ID.to_string(),
        bytes: &LOCKUP_WASM_BYTES,
        signer_account: root,
        deposit: MIN_BALANCE_FOR_STORAGE + lockup_amount,
        gas: MAX_GAS,
        init_method: new(
            owner.account_id.clone(),
            1000000000.into(),
            None,
            TransfersInformation::TransfersDisabled {
                transfer_poll_account_id: "transfer-poll".to_string(),
            },
            Some(VestingScheduleOrHash::VestingHash(
                near_sdk_sim::hash::hash(
                    &VestingScheduleWithSalt {
                        vesting_schedule: vesting_schedule.clone(),
                        salt: salt.clone().into(),
                    }
                    .try_to_vec()
                    .unwrap(),
                )
                .as_ref()
                .to_vec()
                .into(),
            )),
            None,
            STAKING_POOL_WHITELIST_ACCOUNT_ID.to_string(),
            Some(foundation.account_id.clone())
        )
    );

    let owner_staking_account = &owner;

    let res: Option<AccountId> = owner
        .view_method_call(lockup.contract.get_staking_pool_account_id())
        .unwrap_json();
    assert_eq!(res, None);

    // Selecting staking pool
    owner_staking_account
        .function_call(
            lockup
                .contract
                .select_staking_pool(STAKING_POOL_ACCOUNT_ID.to_string()),
            MAX_GAS,
            0,
        )
        .assert_success();

    let res: Option<AccountId> = owner
        .view_method_call(lockup.contract.get_staking_pool_account_id())
        .unwrap_json();
    assert_eq!(res, Some(STAKING_POOL_ACCOUNT_ID.to_string()));
    let res: U128 = owner
        .view_method_call(lockup.contract.get_known_deposited_balance())
        .unwrap_json();
    assert_eq!(res.0, 0);

    // Depositing and staking on the staking pool
    let staking_amount = lockup_amount - to_yocto("100");
    owner_staking_account
        .function_call(
            lockup.contract.deposit_and_stake(U128(staking_amount)),
            MAX_GAS,
            0,
        )
        .assert_success();

    let res: U128 = owner
        .view_method_call(lockup.contract.get_known_deposited_balance())
        .unwrap_json();
    assert_eq!(res.0, staking_amount);

    // Simulating rewards
    foundation
        .transfer(STAKING_POOL_ACCOUNT_ID.to_string(), to_yocto("10"))
        .assert_success();

    // Pinging the staking pool
    foundation
        .call(STAKING_POOL_ACCOUNT_ID.to_string(), "ping", b"", MAX_GAS, 0)
        .assert_success();

    let res: U128 = owner
        .view_method_call(
            lockup
                .contract
                .get_locked_vested_amount(vesting_schedule.clone()),
        )
        .unwrap_json();
    assert_eq!(res.0, 0);

    // Updating the timestamp to simulate some vesting
    root.borrow_runtime_mut().cur_block.block_timestamp = start_timestamp + 1500;

    let res: U128 = owner
        .view_method_call(
            lockup
                .contract
                .get_locked_vested_amount(vesting_schedule.clone()),
        )
        .unwrap_json();
    assert_eq!(res.0, (lockup_amount + MIN_BALANCE_FOR_STORAGE) * 3 / 8);

    let res: U128 = owner
        .view_method_call(
            lockup
                .contract
                .get_unvested_amount(vesting_schedule.clone()),
        )
        .unwrap_json();
    assert_eq!(res.0, (lockup_amount + MIN_BALANCE_FOR_STORAGE) * 5 / 8);

    // Terminating the vesting schedule

    let res: Option<TerminationStatus> = owner
        .view_method_call(lockup.contract.get_termination_status())
        .unwrap_json();
    assert!(res.is_none());

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_terminated_unvested_balance())
        .unwrap_json();
    assert_eq!(res.0, 0);

    foundation
        .function_call(
            lockup
                .contract
                .terminate_vesting(Some(VestingScheduleWithSalt {
                    vesting_schedule: vesting_schedule.clone(),
                    salt: salt.clone().into(),
                })),
            MAX_GAS,
            0,
        )
        .assert_success();

    let res: Option<TerminationStatus> = owner
        .view_method_call(lockup.contract.get_termination_status())
        .unwrap_json();
    assert_eq!(res, Some(TerminationStatus::VestingTerminatedWithDeficit));

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_terminated_unvested_balance())
        .unwrap_json();
    let unvested_balance = (lockup_amount + MIN_BALANCE_FOR_STORAGE) * 5 / 8;
    assert_eq!(res.0, unvested_balance);

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_terminated_unvested_balance_deficit())
        .unwrap_json();
    // The rest of the tokens are on the staking pool.
    assert_eq_with_gas(res.0, unvested_balance - to_yocto("100"));

    let res: U128 = owner
        .view(
            STAKING_POOL_ACCOUNT_ID.to_string(),
            "get_account_staked_balance",
            &json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })
                .to_string()
                .into_bytes(),
        )
        .unwrap_json();
    assert!(res.0 > 0);

    foundation
        .function_call(
            lockup.contract.termination_prepare_to_withdraw(),
            MAX_GAS,
            0,
        )
        .assert_success();

    let res: Option<TerminationStatus> = owner
        .view_method_call(lockup.contract.get_termination_status())
        .unwrap_json();
    assert_eq!(res, Some(TerminationStatus::EverythingUnstaked));

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_terminated_unvested_balance_deficit())
        .unwrap_json();
    assert_eq_with_gas(res.0, unvested_balance - to_yocto("100"));

    let res: U128 = owner
        .view(
            STAKING_POOL_ACCOUNT_ID.to_string(),
            "get_account_staked_balance",
            &json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })
                .to_string()
                .into_bytes(),
        )
        .unwrap_json();
    assert_eq_with_gas(res.0, 0);

    let res: U128 = owner
        .view(
            STAKING_POOL_ACCOUNT_ID.to_string(),
            "get_account_unstaked_balance",
            &json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })
                .to_string()
                .into_bytes(),
        )
        .unwrap_json();
    assert!(res.0 > 0);

    let res: bool = foundation
        .function_call(
            lockup.contract.termination_prepare_to_withdraw(),
            MAX_GAS,
            0,
        )
        .unwrap_json();
    // Need to wait 4 epochs
    assert!(!res);

    let res: Option<TerminationStatus> = owner
        .view_method_call(lockup.contract.get_termination_status())
        .unwrap_json();
    assert_eq!(res, Some(TerminationStatus::EverythingUnstaked));

    root.borrow_runtime_mut().cur_block.block_height += 40;
    root.borrow_runtime_mut().cur_block.epoch_height += 4;

    // The standalone runtime doesn't unlock locked balance. Need to manually intervene.
    let mut pool = staking_pool.account().unwrap();
    pool.amount += pool.locked;
    pool.locked = 0;
    staking_pool
        .borrow_runtime_mut()
        .force_account_update(STAKING_POOL_ACCOUNT_ID.to_string(), &pool);

    let res: U128 = owner
        .view(
            STAKING_POOL_ACCOUNT_ID.to_string(),
            "get_account_unstaked_balance",
            &json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })
                .to_string()
                .into_bytes(),
        )
        .unwrap_json();
    let received_reward = res.0 - staking_amount;

    let res: bool = foundation
        .function_call(
            lockup.contract.termination_prepare_to_withdraw(),
            MAX_GAS,
            0,
        )
        .unwrap_json();
    assert!(res);

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_terminated_unvested_balance_deficit())
        .unwrap_json();
    assert_eq!(res.0, 0);

    let res: Option<TerminationStatus> = owner
        .view_method_call(lockup.contract.get_termination_status())
        .unwrap_json();
    assert_eq!(res, Some(TerminationStatus::ReadyToWithdraw));

    let res: U128 = owner
        .view(
            STAKING_POOL_ACCOUNT_ID.to_string(),
            "get_account_unstaked_balance",
            &json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })
                .to_string()
                .into_bytes(),
        )
        .unwrap_json();
    assert_eq!(res.0, 0);

    let res: U128 = owner
        .view_method_call(lockup.contract.get_known_deposited_balance())
        .unwrap_json();
    assert_eq!(res.0, 0);

    let foundation_balance = foundation.account().unwrap().amount;

    let res: bool = foundation
        .function_call(
            lockup
                .contract
                .termination_withdraw(foundation.account_id.clone()),
            MAX_GAS,
            0,
        )
        .unwrap_json();
    assert!(res);

    let res: Option<TerminationStatus> = owner
        .view_method_call(lockup.contract.get_termination_status())
        .unwrap_json();
    assert_eq!(res, None);

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_terminated_unvested_balance())
        .unwrap_json();
    assert_eq!(res.0, 0);

    let new_foundation_balance = foundation.account().unwrap().amount;
    assert_eq_with_gas(
        new_foundation_balance,
        foundation_balance + unvested_balance,
    );

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_locked_amount())
        .unwrap_json();
    assert_eq!(res.0, (lockup_amount + MIN_BALANCE_FOR_STORAGE) - unvested_balance);

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_liquid_owners_balance())
        .unwrap_json();
    assert_eq_with_gas(res.0, received_reward);

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_balance())
        .unwrap_json();
    assert_eq_with_gas(
        res.0,
        (lockup_amount + MIN_BALANCE_FOR_STORAGE) - unvested_balance + received_reward,
    );
}

#[test]
fn termination_with_staking() {
    let lockup_amount = to_yocto("1000");
    let (root, foundation, owner, staking_pool) = basic_setup();

    let start_timestamp = root.borrow_runtime().cur_block.block_timestamp;

    let vesting_schedule = VestingSchedule {
        start_timestamp: start_timestamp.into(),
        cliff_timestamp: (start_timestamp + 1000).into(),
        end_timestamp: (start_timestamp + 4000).into(),
    };

    let lockup = deploy!(
        contract: LockupContractContract,
        contract_id: LOCKUP_ACCOUNT_ID.to_string(),
        bytes: &LOCKUP_WASM_BYTES,
        signer_account: root,
        deposit: MIN_BALANCE_FOR_STORAGE + lockup_amount,
        gas: MAX_GAS,
        init_method: new(
            owner.account_id.clone(),
            1000000000.into(),
            None,
            TransfersInformation::TransfersDisabled {
                transfer_poll_account_id: "transfer-poll".to_string(),
            },
            Some(VestingScheduleOrHash::VestingSchedule(
                vesting_schedule.clone(),
            )),
            None,
            STAKING_POOL_WHITELIST_ACCOUNT_ID.to_string(),
            Some(foundation.account_id.clone())
        )
    );

    let owner_staking_account = &owner;

    let res: Option<AccountId> = owner
        .view_method_call(lockup.contract.get_staking_pool_account_id())
        .unwrap_json();
    assert_eq!(res, None);

    // Selecting staking pool
    owner_staking_account
        .function_call(
            lockup
                .contract
                .select_staking_pool(STAKING_POOL_ACCOUNT_ID.to_string()),
            MAX_GAS,
            0,
        )
        .assert_success();

    let res: Option<AccountId> = owner
        .view_method_call(lockup.contract.get_staking_pool_account_id())
        .unwrap_json();
    assert_eq!(res, Some(STAKING_POOL_ACCOUNT_ID.to_string()));
    let res: U128 = owner
        .view_method_call(lockup.contract.get_known_deposited_balance())
        .unwrap_json();
    assert_eq!(res.0, 0);

    // Depositing and staking on the staking pool
    let staking_amount = lockup_amount - to_yocto("100");
    owner_staking_account
        .function_call(
            lockup.contract.deposit_and_stake(U128(staking_amount)),
            MAX_GAS,
            0,
        )
        .assert_success();

    let res: U128 = owner
        .view_method_call(lockup.contract.get_known_deposited_balance())
        .unwrap_json();
    assert_eq!(res.0, staking_amount);

    // Simulating rewards
    foundation
        .transfer(STAKING_POOL_ACCOUNT_ID.to_string(), to_yocto("10"))
        .assert_success();

    // Pinging the staking pool
    foundation
        .call(STAKING_POOL_ACCOUNT_ID.to_string(), "ping", b"", MAX_GAS, 0)
        .assert_success();

    let res: U128 = owner
        .view_method_call(
            lockup
                .contract
                .get_locked_vested_amount(vesting_schedule.clone()),
        )
        .unwrap_json();
    assert_eq!(res.0, 0);

    // Updating the timestamp to simulate some vesting
    root.borrow_runtime_mut().cur_block.block_timestamp = start_timestamp + 1500;

    let res: U128 = owner
        .view_method_call(
            lockup
                .contract
                .get_locked_vested_amount(vesting_schedule.clone()),
        )
        .unwrap_json();
    assert_eq!(res.0, (lockup_amount + MIN_BALANCE_FOR_STORAGE) * 3 / 8);

    let res: U128 = owner
        .view_method_call(
            lockup
                .contract
                .get_unvested_amount(vesting_schedule.clone()),
        )
        .unwrap_json();
    assert_eq!(res.0, (lockup_amount + MIN_BALANCE_FOR_STORAGE) * 5 / 8);

    // Terminating the vesting schedule

    let res: Option<TerminationStatus> = owner
        .view_method_call(lockup.contract.get_termination_status())
        .unwrap_json();
    assert!(res.is_none());

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_terminated_unvested_balance())
        .unwrap_json();
    assert_eq!(res.0, 0);

    foundation
        .function_call(lockup.contract.terminate_vesting(None), MAX_GAS, 0)
        .assert_success();

    let res: Option<TerminationStatus> = owner
        .view_method_call(lockup.contract.get_termination_status())
        .unwrap_json();
    assert_eq!(res, Some(TerminationStatus::VestingTerminatedWithDeficit));

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_terminated_unvested_balance())
        .unwrap_json();
    let unvested_balance = (lockup_amount + MIN_BALANCE_FOR_STORAGE) * 5 / 8;
    assert_eq!(res.0, unvested_balance);

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_terminated_unvested_balance_deficit())
        .unwrap_json();
    // The rest of the tokens are on the staking pool.
    assert_eq_with_gas(res.0, unvested_balance - to_yocto("100"));

    let res: U128 = owner
        .view(
            STAKING_POOL_ACCOUNT_ID.to_string(),
            "get_account_staked_balance",
            &json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })
                .to_string()
                .into_bytes(),
        )
        .unwrap_json();
    assert!(res.0 > 0);

    foundation
        .function_call(
            lockup.contract.termination_prepare_to_withdraw(),
            MAX_GAS,
            0,
        )
        .assert_success();

    let res: Option<TerminationStatus> = owner
        .view_method_call(lockup.contract.get_termination_status())
        .unwrap_json();
    assert_eq!(res, Some(TerminationStatus::EverythingUnstaked));

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_terminated_unvested_balance_deficit())
        .unwrap_json();
    assert_eq_with_gas(res.0, unvested_balance - to_yocto("100"));

    let res: U128 = owner
        .view(
            STAKING_POOL_ACCOUNT_ID.to_string(),
            "get_account_staked_balance",
            &json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })
                .to_string()
                .into_bytes(),
        )
        .unwrap_json();
    assert_eq_with_gas(res.0, 0);

    let res: U128 = owner
        .view(
            STAKING_POOL_ACCOUNT_ID.to_string(),
            "get_account_unstaked_balance",
            &json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })
                .to_string()
                .into_bytes(),
        )
        .unwrap_json();
    assert!(res.0 > 0);

    let res: bool = foundation
        .function_call(
            lockup.contract.termination_prepare_to_withdraw(),
            MAX_GAS,
            0,
        )
        .unwrap_json();
    // Need to wait 4 epochs
    assert!(!res);

    let res: Option<TerminationStatus> = owner
        .view_method_call(lockup.contract.get_termination_status())
        .unwrap_json();
    assert_eq!(res, Some(TerminationStatus::EverythingUnstaked));

    root.borrow_runtime_mut().cur_block.block_height += 40;
    root.borrow_runtime_mut().cur_block.epoch_height += 4;

    // The standalone runtime doesn't unlock locked balance. Need to manually intervene.
    let mut pool = staking_pool.account().unwrap();
    pool.amount += pool.locked;
    pool.locked = 0;
    staking_pool
        .borrow_runtime_mut()
        .force_account_update(STAKING_POOL_ACCOUNT_ID.to_string(), &pool);

    let res: U128 = owner
        .view(
            STAKING_POOL_ACCOUNT_ID.to_string(),
            "get_account_unstaked_balance",
            &json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })
                .to_string()
                .into_bytes(),
        )
        .unwrap_json();
    let received_reward = res.0 - staking_amount;

    let res: bool = foundation
        .function_call(
            lockup.contract.termination_prepare_to_withdraw(),
            MAX_GAS,
            0,
        )
        .unwrap_json();
    assert!(res);

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_terminated_unvested_balance_deficit())
        .unwrap_json();
    assert_eq!(res.0, 0);

    let res: Option<TerminationStatus> = owner
        .view_method_call(lockup.contract.get_termination_status())
        .unwrap_json();
    assert_eq!(res, Some(TerminationStatus::ReadyToWithdraw));

    let res: U128 = owner
        .view(
            STAKING_POOL_ACCOUNT_ID.to_string(),
            "get_account_unstaked_balance",
            &json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })
                .to_string()
                .into_bytes(),
        )
        .unwrap_json();
    assert_eq!(res.0, 0);

    let res: U128 = owner
        .view_method_call(lockup.contract.get_known_deposited_balance())
        .unwrap_json();
    assert_eq!(res.0, 0);

    let foundation_balance = foundation.account().unwrap().amount;

    let res: bool = foundation
        .function_call(
            lockup
                .contract
                .termination_withdraw(foundation.account_id.clone()),
            MAX_GAS,
            0,
        )
        .unwrap_json();
    assert!(res);

    let res: Option<TerminationStatus> = owner
        .view_method_call(lockup.contract.get_termination_status())
        .unwrap_json();
    assert_eq!(res, None);

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_terminated_unvested_balance())
        .unwrap_json();
    assert_eq!(res.0, 0);

    let new_foundation_balance = foundation.account().unwrap().amount;
    assert_eq_with_gas(
        new_foundation_balance,
        foundation_balance + unvested_balance,
    );

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_locked_amount())
        .unwrap_json();
    assert_eq!(res.0, (lockup_amount + MIN_BALANCE_FOR_STORAGE) - unvested_balance);

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_liquid_owners_balance())
        .unwrap_json();
    assert_eq_with_gas(res.0, received_reward);

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_balance())
        .unwrap_json();
    assert_eq_with_gas(
        res.0,
        (lockup_amount + MIN_BALANCE_FOR_STORAGE) - unvested_balance + received_reward,
    );
}

#[test]
fn test_release_schedule_unlock_transfers() {
    let lockup_amount = to_yocto("1000");
    let (root, foundation, owner, _staking_pool) = basic_setup();

    // Initializing fake voting contract
    let _voting = root.deploy(
        &FAKE_VOTING_WASM_BYTES,
        TRANSFER_POLL_ACCOUNT_ID.to_string(),
        to_yocto("30"),
    );

    // Unlock timestamp from fake voting contract.
    let unlock_timestamp = 1535760000000000000u64;
    root.borrow_runtime_mut().cur_block.block_timestamp = unlock_timestamp + 1000;

    let lockup = deploy!(
        contract: LockupContractContract,
        contract_id: LOCKUP_ACCOUNT_ID.to_string(),
        bytes: &LOCKUP_WASM_BYTES,
        signer_account: root,
        deposit: MIN_BALANCE_FOR_STORAGE + lockup_amount,
        gas: MAX_GAS,
        init_method: new(
            owner.account_id.clone(),
            0.into(),
            None,
            TransfersInformation::TransfersDisabled {
                transfer_poll_account_id: TRANSFER_POLL_ACCOUNT_ID.to_string(),
            },
            None,
            Some(1000000000000.into()),
            STAKING_POOL_WHITELIST_ACCOUNT_ID.to_string(),
            None
        )
    );

    let owner_staking_account = &owner;

    let res: Option<AccountId> = owner
        .view_method_call(lockup.contract.get_staking_pool_account_id())
        .unwrap_json();
    assert_eq!(res, None);

    // Selecting staking pool
    owner_staking_account
        .function_call(
            lockup
                .contract
                .select_staking_pool(STAKING_POOL_ACCOUNT_ID.to_string()),
            MAX_GAS,
            0,
        )
        .assert_success();

    let res: Option<AccountId> = owner
        .view_method_call(lockup.contract.get_staking_pool_account_id())
        .unwrap_json();
    assert_eq!(res, Some(STAKING_POOL_ACCOUNT_ID.to_string()));
    let res: U128 = owner
        .view_method_call(lockup.contract.get_known_deposited_balance())
        .unwrap_json();
    assert_eq!(res.0, 0);

    // Depositing and staking on the staking pool
    let staking_amount = lockup_amount - to_yocto("100");
    owner_staking_account
        .function_call(
            lockup.contract.deposit_and_stake(U128(staking_amount)),
            MAX_GAS,
            0,
        )
        .assert_success();

    let res: U128 = owner
        .view_method_call(lockup.contract.get_known_deposited_balance())
        .unwrap_json();
    assert_eq!(res.0, staking_amount);

    // Simulating rewards
    foundation
        .transfer(STAKING_POOL_ACCOUNT_ID.to_string(), to_yocto("10"))
        .assert_success();

    // Pinging the staking pool
    foundation
        .call(STAKING_POOL_ACCOUNT_ID.to_string(), "ping", b"", MAX_GAS, 0)
        .assert_success();

    let res: U128 = owner
        .view(
            STAKING_POOL_ACCOUNT_ID.to_string(),
            "get_account_staked_balance",
            &json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })
                .to_string()
                .into_bytes(),
        )
        .unwrap_json();
    let received_reward = res.0 - staking_amount;

    owner_staking_account
        .function_call(lockup.contract.refresh_staking_pool_balance(), MAX_GAS, 0)
        .assert_success();

    let full_lockup_amount = lockup_amount + MIN_BALANCE_FOR_STORAGE;

    // Reset timestamp to 0, to avoid any release
    root.borrow_runtime_mut().cur_block.block_timestamp = unlock_timestamp;

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_locked_amount())
        .unwrap_json();
    assert_eq!(res.0, full_lockup_amount);

    let res: U128 = owner
        .view_method_call(lockup.contract.get_known_deposited_balance())
        .unwrap_json();
    assert_eq_with_gas(res.0, staking_amount + received_reward);

    let res: bool = owner
        .view_method_call(lockup.contract.are_transfers_enabled())
        .unwrap_json();
    assert!(!res);

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_liquid_owners_balance())
        .unwrap_json();
    assert_eq_with_gas(res.0, received_reward);

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_owners_balance())
        .unwrap_json();
    assert_eq_with_gas(res.0, received_reward);

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_balance())
        .unwrap_json();
    assert_eq_with_gas(res.0, full_lockup_amount + received_reward);

    let transfer_amount = to_yocto("5");
    assert!(transfer_amount < received_reward);
    let owner_balance = owner.account().unwrap().amount;

    assert!(!owner_staking_account
        .function_call(
            lockup
                .contract
                .transfer(U128(transfer_amount), owner.account_id.clone()),
            MAX_GAS,
            0,
        )
        .is_ok());

    let new_owner_balance = owner.account().unwrap().amount;
    assert_eq_with_gas(new_owner_balance, owner_balance);

    let res: bool = owner_staking_account
        .function_call(lockup.contract.check_transfers_vote(), MAX_GAS, 0)
        .unwrap_json();
    assert!(res);

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_locked_amount())
        .unwrap_json();
    assert_eq!(res.0, lockup_amount + MIN_BALANCE_FOR_STORAGE);

    let res: bool = owner
        .view_method_call(lockup.contract.are_transfers_enabled())
        .unwrap_json();
    assert!(res);

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_liquid_owners_balance())
        .unwrap_json();
    assert_eq_with_gas(res.0, received_reward);

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_owners_balance())
        .unwrap_json();
    assert_eq_with_gas(res.0, received_reward);

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_balance())
        .unwrap_json();
    assert_eq_with_gas(res.0, full_lockup_amount + received_reward);

    owner_staking_account
        .function_call(
            lockup
                .contract
                .transfer(U128(transfer_amount), owner.account_id.clone()),
            MAX_GAS,
            0,
        )
        .assert_success();

    let new_owner_balance = owner.account().unwrap().amount;
    assert_eq_with_gas(new_owner_balance, owner_balance + transfer_amount);

    let liquid_balance = received_reward - transfer_amount;

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_locked_amount())
        .unwrap_json();
    assert_eq!(res.0, full_lockup_amount);

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_liquid_owners_balance())
        .unwrap_json();
    assert_eq_with_gas(res.0, liquid_balance);

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_owners_balance())
        .unwrap_json();
    assert_eq_with_gas(res.0, liquid_balance);

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_balance())
        .unwrap_json();
    assert_eq_with_gas(res.0, full_lockup_amount + liquid_balance);

    let transfer_amount = to_yocto("15");
    assert!(transfer_amount > liquid_balance);

    let owner_balance = new_owner_balance;

    assert!(!owner_staking_account
        .function_call(
            lockup
                .contract
                .transfer(U128(transfer_amount), owner.account_id.clone()),
            MAX_GAS,
            0,
        )
        .is_ok());

    let new_owner_balance = owner.account().unwrap().amount;
    assert_eq_with_gas(new_owner_balance, owner_balance);

    // At this timestamp only 1/1000 of the lockup_amount is released.
    root.borrow_runtime_mut().cur_block.block_timestamp = unlock_timestamp + 1000000000;

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_locked_amount())
        .unwrap_json();
    assert_eq!(res.0, full_lockup_amount - full_lockup_amount / 1000);

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_liquid_owners_balance())
        .unwrap_json();
    assert_eq_with_gas(res.0, liquid_balance + full_lockup_amount / 1000);

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_owners_balance())
        .unwrap_json();
    assert_eq_with_gas(res.0, liquid_balance + full_lockup_amount / 1000);

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_balance())
        .unwrap_json();
    assert_eq_with_gas(res.0, full_lockup_amount + liquid_balance);

    // Adding more time. So 50/1000 is unlocked
    root.borrow_runtime_mut().cur_block.block_timestamp = unlock_timestamp + 50_000_000_000;

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_locked_amount())
        .unwrap_json();
    assert_eq!(res.0, full_lockup_amount - 50 * full_lockup_amount / 1000);

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_liquid_owners_balance())
        .unwrap_json();
    assert_eq_with_gas(res.0, liquid_balance + 50 * full_lockup_amount / 1000);

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_owners_balance())
        .unwrap_json();
    assert_eq_with_gas(res.0, liquid_balance + 50 * full_lockup_amount / 1000);

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_balance())
        .unwrap_json();
    assert_eq_with_gas(res.0, full_lockup_amount + liquid_balance);

    // Transferring 15 more
    owner_staking_account
        .function_call(
            lockup
                .contract
                .transfer(U128(transfer_amount), owner.account_id.clone()),
            MAX_GAS,
            0,
        )
        .assert_success();

    let new_owner_balance = owner.account().unwrap().amount;
    assert_eq_with_gas(new_owner_balance, owner_balance + transfer_amount);

    let full_balance = full_lockup_amount + liquid_balance - transfer_amount;
    let liquid_balance = liquid_balance + 51 * full_lockup_amount / 1000 - transfer_amount;

    // Setting time to 51/1000 to have round numbers
    root.borrow_runtime_mut().cur_block.block_timestamp = unlock_timestamp + 51_000_000_000;

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_locked_amount())
        .unwrap_json();
    assert_eq!(res.0, full_lockup_amount - 51 * full_lockup_amount / 1000);

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_liquid_owners_balance())
        .unwrap_json();
    assert_eq_with_gas(res.0, liquid_balance);

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_owners_balance())
        .unwrap_json();
    assert_eq_with_gas(res.0, liquid_balance);

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_balance())
        .unwrap_json();
    assert_eq_with_gas(res.0, full_balance);

    // Setting time to 200/1000 to check liquid balance, because majority of balance is still staked.
    root.borrow_runtime_mut().cur_block.block_timestamp = unlock_timestamp + 200_000_000_000;
    let owners_balance = liquid_balance + 149 * full_lockup_amount / 1000;

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_locked_amount())
        .unwrap_json();
    assert_eq!(res.0, full_lockup_amount - 200 * full_lockup_amount / 1000);

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_liquid_owners_balance())
        .unwrap_json();
    // The account balance is `100`. `+3.5` for storage and `-20` for transfers.
    assert_eq_with_gas(res.0, to_yocto("80"));

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_owners_balance())
        .unwrap_json();
    assert_eq_with_gas(res.0, owners_balance);

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_balance())
        .unwrap_json();
    assert_eq_with_gas(res.0, full_balance);

    let public_key: Base58PublicKey = owner_staking_account
        .signer
        .public_key
        .try_to_vec()
        .unwrap()
        .try_into()
        .unwrap();

    // Trying to add full access key.
    assert!(!owner_staking_account
        .function_call(
            lockup.contract.add_full_access_key(public_key.clone()),
            MAX_GAS,
            0
        )
        .is_ok());

    // Setting time to full release.
    root.borrow_runtime_mut().cur_block.block_timestamp = unlock_timestamp + 1100_000_000_000;

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_locked_amount())
        .unwrap_json();
    assert_eq!(res.0, 0);

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_liquid_owners_balance())
        .unwrap_json();
    assert_eq_with_gas(res.0, to_yocto("80"));

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_owners_balance())
        .unwrap_json();
    assert_eq_with_gas(res.0, full_balance);

    let res: WrappedBalance = owner
        .view_method_call(lockup.contract.get_balance())
        .unwrap_json();
    assert_eq_with_gas(res.0, full_balance);

    // Adding full access key
    owner_staking_account
        .function_call(
            lockup.contract.add_full_access_key(public_key.clone()),
            MAX_GAS,
            0,
        )
        .assert_success();

    let mut lockup_account = root.create_user("tmp".to_string(), to_yocto("100"));
    lockup_account.account_id = LOCKUP_ACCOUNT_ID.to_string();
    lockup_account.signer = owner.signer.clone();

    // Testing direct transfer
    let owner_balance = owner.account().unwrap().amount;
    lockup_account
        .transfer(owner.account_id.clone(), transfer_amount)
        .assert_success();

    let new_owner_balance = owner.account().unwrap().amount;
    assert_eq!(new_owner_balance, owner_balance + transfer_amount);
}

fn basic_setup() -> (UserAccount, UserAccount, UserAccount, UserAccount) {
    let mut genesis_config = GenesisConfig::default();
    genesis_config.block_prod_time = 0;
    let root = init_simulator(Some(genesis_config));

    let foundation = root.create_user("foundation".to_string(), to_yocto("10000"));

    let owner = root.create_user("owner".to_string(), to_yocto("30"));

    // Creating whitelist account
    let _whitelist = root.deploy_and_init(
        &WHITELIST_WASM_BYTES,
        STAKING_POOL_WHITELIST_ACCOUNT_ID.to_string(),
        "new",
        &json!({
            "foundation_account_id": foundation.valid_account_id(),
        })
        .to_string()
        .into_bytes(),
        to_yocto("30"),
        MAX_GAS,
    );

    // Whitelisting staking pool
    foundation
        .call(
            STAKING_POOL_WHITELIST_ACCOUNT_ID.to_string(),
            "add_staking_pool",
            &json!({
                "staking_pool_account_id": STAKING_POOL_ACCOUNT_ID.to_string(),
            })
            .to_string()
            .into_bytes(),
            MAX_GAS,
            NO_DEPOSIT,
        )
        .assert_success();

    // Creating staking pool
    let staking_pool = root.deploy_and_init(
        &STAKING_POOL_WASM_BYTES,
        STAKING_POOL_ACCOUNT_ID.to_string(),
        "new",
        &json!({
            "owner_id": foundation.valid_account_id(),
            "stake_public_key": "ed25519:3tysLvy7KGoE8pznUgXvSHa4vYyGvrDZFcT8jgb8PEQ6",
            "reward_fee_fraction": {
                "numerator": 10,
                "denominator": 100
            }
        })
        .to_string()
        .into_bytes(),
        to_yocto("40"),
        MAX_GAS,
    );

    (root, foundation, owner, staking_pool)
}
