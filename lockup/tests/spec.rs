extern crate quickcheck;
#[macro_use(quickcheck)]
extern crate quickcheck_macros;

mod utils;

use crate::utils::{call_view, wait_epoch, ExternalUser, LOCKUP_ACCOUNT_ID};
use lockup_contract::{
    TerminationStatus, TransfersInformation, VestingSchedule, VestingScheduleOrHash, WrappedBalance,
};
use near_primitives::transaction::ExecutionStatus;
use near_primitives::types::Balance;
use near_runtime_standalone::RuntimeStandalone;
use near_sdk::borsh::BorshSerialize;
use near_sdk::json_types::{Base58PublicKey, Base64VecU8, U128};
use near_sdk::serde_json::{self, json};
use near_sdk::AccountId;
use std::convert::TryInto;
use utils::{call_lockup, new_root, ntoy, InitLockupArgs};

pub fn hash_vesting_schedule(vesting_schedule: &VestingSchedule, salt: &[u8]) -> Vec<u8> {
    near_primitives::hash::hash(
        &[
            vesting_schedule.try_to_vec().expect("Failed to serialize"),
            salt.to_vec(),
        ]
        .concat(),
    )
    .try_to_vec()
    .unwrap()
}

#[quickcheck]
fn lockup(lockup_amount: Balance, lockup_duration: u64, lockup_timestamp: u64) {
    let (mut r, foundation, owner) = basic_setup();

    let args = InitLockupArgs {
        owner_account_id: owner.account_id.clone(),
        lockup_duration: lockup_duration.into(),
        lockup_timestamp: None,
        transfers_information: TransfersInformation::TransfersEnabled {
            transfers_timestamp: lockup_timestamp.saturating_add(1).into(),
        },
        vesting_schedule: None,
        release_duration: None,
        foundation_account_id: None,
        staking_pool_whitelist_account_id: "staking".into(),
    };

    foundation
        .init_lockup(&mut r, &args, lockup_amount)
        .unwrap();

    r.current_block().block_timestamp = lockup_timestamp
        .saturating_add(lockup_duration)
        .saturating_sub(1);

    let locked_amount: U128 = call_lockup(&r, "get_locked_amount", "");
    assert_eq!(locked_amount.0, ntoy(35) + lockup_amount);

    r.current_block().block_timestamp = r.current_block().block_timestamp.saturating_add(2);

    let locked_amount: U128 = call_lockup(&r, "get_locked_amount", "");
    assert_eq!(locked_amount.0, 0);
}

#[test]
fn staking() {
    let lockup_amount = ntoy(1000);
    let (mut r, foundation, owner) = basic_setup();

    let staking_pool_whitelist_account_id = "staking-pool-whitelist".to_string();
    let staking_pool_account_id = "staking-pool".to_string();

    // Creating whitelist account
    foundation
        .init_whitelist(&mut r, staking_pool_whitelist_account_id.clone())
        .unwrap();

    // Whitelisting staking pool
    foundation
        .function_call(
            &mut r,
            &staking_pool_whitelist_account_id,
            "add_staking_pool",
            &serde_json::to_vec(
                &json!({"staking_pool_account_id": staking_pool_account_id.clone()}),
            )
            .unwrap(),
        )
        .unwrap();

    // Creating staking pool
    foundation
        .init_staking_pool(&mut r, staking_pool_account_id.clone())
        .unwrap();

    // Whitelisting staking pool
    foundation
        .function_call(
            &mut r,
            &staking_pool_whitelist_account_id,
            "add_staking_pool",
            &serde_json::to_vec(
                &json!({"staking_pool_account_id": staking_pool_account_id.clone()}),
            )
            .unwrap(),
        )
        .unwrap();

    let args = InitLockupArgs {
        owner_account_id: owner.account_id.clone(),
        lockup_duration: 1000000000.into(),
        lockup_timestamp: None,
        transfers_information: TransfersInformation::TransfersDisabled {
            transfer_poll_account_id: "transfer-poll".to_string(),
        },
        vesting_schedule: None,
        release_duration: None,
        foundation_account_id: None,
        staking_pool_whitelist_account_id: staking_pool_whitelist_account_id.clone(),
    };

    foundation
        .init_lockup(&mut r, &args, lockup_amount)
        .unwrap();

    let owner_staking_account = owner.clone();

    let res: Option<AccountId> = call_lockup(&r, "get_staking_pool_account_id", "");
    assert_eq!(res, None);

    // Selecting staking pool
    owner_staking_account
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "select_staking_pool",
            &serde_json::to_vec(
                &json!({"staking_pool_account_id": staking_pool_account_id.clone()}),
            )
            .unwrap(),
        )
        .unwrap();

    let res: Option<AccountId> = call_lockup(&r, "get_staking_pool_account_id", "");
    assert_eq!(res, Some(staking_pool_account_id.clone()));
    let res: U128 = call_lockup(&r, "get_known_deposited_balance", "");
    assert_eq!(res.0, 0);

    // Depositing to the staking pool
    let staking_amount = lockup_amount - ntoy(100);
    owner_staking_account
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "deposit_to_staking_pool",
            &serde_json::to_vec(&json!({ "amount": U128(staking_amount) })).unwrap(),
        )
        .unwrap();

    let res: U128 = call_lockup(&r, "get_known_deposited_balance", "");
    assert_eq!(res.0, staking_amount);

    // Staking on the staking pool
    owner_staking_account
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "stake",
            &serde_json::to_vec(&json!({ "amount": U128(staking_amount) })).unwrap(),
        )
        .unwrap();

    let res: U128 = call_view(
        &r,
        &staking_pool_account_id.clone(),
        "get_account_staked_balance",
        &serde_json::to_string(&json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })).unwrap(),
    );
    assert_eq!(res.0, staking_amount);

    // Refreshing staking balance. Should be NOOP
    owner_staking_account
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "refresh_staking_pool_balance",
            &[],
        )
        .unwrap();

    let res: U128 = call_lockup(&r, "get_known_deposited_balance", "");
    assert_eq!(res.0, staking_amount);

    // Simulating rewards
    foundation
        .transfer(&mut r, &staking_pool_account_id, ntoy(10))
        .unwrap();

    // Pinging the staking pool
    foundation
        .function_call(&mut r, &staking_pool_account_id, "ping", &[])
        .unwrap();

    let res: U128 = call_view(
        &r,
        &staking_pool_account_id.clone(),
        "get_account_staked_balance",
        &serde_json::to_string(&json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })).unwrap(),
    );
    let new_stake_amount = res.0;
    assert!(new_stake_amount > staking_amount);

    // Refresh staking balance again
    owner_staking_account
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "refresh_staking_pool_balance",
            &[],
        )
        .unwrap();

    let res: U128 = call_lockup(&r, "get_known_deposited_balance", "");
    let new_total_balance = res.0;
    assert!(new_total_balance >= new_stake_amount);

    let res: U128 = call_lockup(&r, "get_owners_balance", "");
    assert_eq!(res.0, new_total_balance - staking_amount);

    // Unstaking everything
    let res = owner_staking_account
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "unstake",
            &serde_json::to_vec(&json!({ "amount": U128(new_stake_amount) })).unwrap(),
        )
        .unwrap();
    assert_eq!(res.status, ExecutionStatus::SuccessValue(b"true".to_vec()));

    let res: U128 = call_view(
        &r,
        &staking_pool_account_id.clone(),
        "get_account_staked_balance",
        &serde_json::to_string(&json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })).unwrap(),
    );
    assert_eq!(res.0, 0);
    let res: U128 = call_view(
        &r,
        &staking_pool_account_id.clone(),
        "get_account_unstaked_balance",
        &serde_json::to_string(&json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })).unwrap(),
    );
    assert!(res.0 >= new_total_balance);

    for _ in 0..4 {
        wait_epoch(&mut r);
    }

    // The standalone runtime doesn't unlock locked balance. Need to manually intervene.
    let mut pool = r.view_account(&staking_pool_account_id).unwrap();
    pool.amount += pool.locked;
    pool.locked = 0;
    r.force_account_update(staking_pool_account_id.clone(), &pool);

    // Withdrawing everything from the staking pool
    let res = owner_staking_account
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "withdraw_from_staking_pool",
            &serde_json::to_vec(&json!({ "amount": U128(new_total_balance) })).unwrap(),
        )
        .unwrap();
    assert_eq!(res.status, ExecutionStatus::SuccessValue(b"true".to_vec()));

    let res: U128 = call_lockup(&r, "get_known_deposited_balance", "");
    assert_eq!(res.0, 0);

    let res: U128 = call_lockup(&r, "get_owners_balance", "");
    assert_eq!(res.0, new_stake_amount - staking_amount);

    // Unselecting the staking pool
    owner_staking_account
        .function_call(&mut r, LOCKUP_ACCOUNT_ID, "unselect_staking_pool", &[])
        .unwrap();

    let res: Option<AccountId> = call_lockup(&r, "get_staking_pool_account_id", "");
    assert_eq!(res, None);
}

#[test]
fn staking_with_helpers() {
    let lockup_amount = ntoy(1000);
    let (mut r, foundation, owner) = basic_setup();

    let staking_pool_whitelist_account_id = "staking-pool-whitelist".to_string();
    let staking_pool_account_id = "staking-pool".to_string();

    // Creating whitelist account
    foundation
        .init_whitelist(&mut r, staking_pool_whitelist_account_id.clone())
        .unwrap();

    // Whitelisting staking pool
    foundation
        .function_call(
            &mut r,
            &staking_pool_whitelist_account_id,
            "add_staking_pool",
            &serde_json::to_vec(
                &json!({"staking_pool_account_id": staking_pool_account_id.clone()}),
            )
            .unwrap(),
        )
        .unwrap();

    // Creating staking pool
    foundation
        .init_staking_pool(&mut r, staking_pool_account_id.clone())
        .unwrap();

    // Whitelisting staking pool
    foundation
        .function_call(
            &mut r,
            &staking_pool_whitelist_account_id,
            "add_staking_pool",
            &serde_json::to_vec(
                &json!({"staking_pool_account_id": staking_pool_account_id.clone()}),
            )
            .unwrap(),
        )
        .unwrap();

    let args = InitLockupArgs {
        owner_account_id: owner.account_id.clone(),
        lockup_duration: 1000000000.into(),
        lockup_timestamp: None,
        transfers_information: TransfersInformation::TransfersDisabled {
            transfer_poll_account_id: "transfer-poll".to_string(),
        },
        vesting_schedule: None,
        release_duration: None,
        foundation_account_id: None,
        staking_pool_whitelist_account_id: staking_pool_whitelist_account_id.clone(),
    };

    foundation
        .init_lockup(&mut r, &args, lockup_amount)
        .unwrap();

    let owner_staking_account = owner.clone();

    let res: Option<AccountId> = call_lockup(&r, "get_staking_pool_account_id", "");
    assert_eq!(res, None);

    // Selecting staking pool
    owner_staking_account
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "select_staking_pool",
            &serde_json::to_vec(
                &json!({"staking_pool_account_id": staking_pool_account_id.clone()}),
            )
            .unwrap(),
        )
        .unwrap();

    let res: Option<AccountId> = call_lockup(&r, "get_staking_pool_account_id", "");
    assert_eq!(res, Some(staking_pool_account_id.clone()));
    let res: U128 = call_lockup(&r, "get_known_deposited_balance", "");
    assert_eq!(res.0, 0);

    // Depositing and staking on the staking pool
    let staking_amount = lockup_amount - ntoy(100);
    owner_staking_account
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "deposit_and_stake",
            &serde_json::to_vec(&json!({ "amount": U128(staking_amount) })).unwrap(),
        )
        .unwrap();

    let res: U128 = call_lockup(&r, "get_known_deposited_balance", "");
    assert_eq!(res.0, staking_amount);

    let res: U128 = call_view(
        &r,
        &staking_pool_account_id.clone(),
        "get_account_staked_balance",
        &serde_json::to_string(&json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })).unwrap(),
    );
    assert_eq!(res.0, staking_amount);

    // Refreshing staking balance. Should be NOOP
    owner_staking_account
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "refresh_staking_pool_balance",
            &[],
        )
        .unwrap();

    let res: U128 = call_lockup(&r, "get_known_deposited_balance", "");
    assert_eq!(res.0, staking_amount);

    // Simulating rewards
    foundation
        .transfer(&mut r, &staking_pool_account_id, ntoy(10))
        .unwrap();

    // Pinging the staking pool
    foundation
        .function_call(&mut r, &staking_pool_account_id, "ping", &[])
        .unwrap();

    let res: U128 = call_view(
        &r,
        &staking_pool_account_id.clone(),
        "get_account_staked_balance",
        &serde_json::to_string(&json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })).unwrap(),
    );
    let new_stake_amount = res.0;
    assert!(new_stake_amount > staking_amount);

    // Refresh staking balance again
    owner_staking_account
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "refresh_staking_pool_balance",
            &[],
        )
        .unwrap();

    let res: U128 = call_lockup(&r, "get_known_deposited_balance", "");
    let new_total_balance = res.0;
    assert!(new_total_balance >= new_stake_amount);

    let res: U128 = call_lockup(&r, "get_owners_balance", "");
    assert_eq!(res.0, new_total_balance - staking_amount);

    // Unstaking everything
    let res = owner_staking_account
        .function_call(&mut r, LOCKUP_ACCOUNT_ID, "unstake_all", b"{}")
        .unwrap();
    assert_eq!(res.status, ExecutionStatus::SuccessValue(b"true".to_vec()));

    let res: U128 = call_view(
        &r,
        &staking_pool_account_id.clone(),
        "get_account_staked_balance",
        &serde_json::to_string(&json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })).unwrap(),
    );
    assert_eq!(res.0, 0);
    let res: U128 = call_view(
        &r,
        &staking_pool_account_id.clone(),
        "get_account_unstaked_balance",
        &serde_json::to_string(&json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })).unwrap(),
    );
    let new_unstaked_amount = res.0;
    assert!(new_unstaked_amount >= new_total_balance);

    for _ in 0..4 {
        wait_epoch(&mut r);
    }

    // The standalone runtime doesn't unlock locked balance. Need to manually intervene.
    let mut pool = r.view_account(&staking_pool_account_id).unwrap();
    pool.amount += pool.locked;
    pool.locked = 0;
    r.force_account_update(staking_pool_account_id.clone(), &pool);

    // Withdrawing everything from the staking pool
    let res = owner_staking_account
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "withdraw_all_from_staking_pool",
            b"{}",
        )
        .unwrap();
    assert_eq!(res.status, ExecutionStatus::SuccessValue(b"true".to_vec()));

    let res: U128 = call_lockup(&r, "get_known_deposited_balance", "");
    assert_eq!(res.0, 0);

    let res: U128 = call_lockup(&r, "get_owners_balance", "");
    assert_eq!(res.0, new_unstaked_amount - staking_amount);

    // Unselecting the staking pool
    owner_staking_account
        .function_call(&mut r, LOCKUP_ACCOUNT_ID, "unselect_staking_pool", &[])
        .unwrap();

    let res: Option<AccountId> = call_lockup(&r, "get_staking_pool_account_id", "");
    assert_eq!(res, None);
}

#[test]
fn termination_with_staking_hashed() {
    let lockup_amount = ntoy(1000);
    let (mut r, foundation, owner) = basic_setup();

    let staking_pool_whitelist_account_id = "staking-pool-whitelist".to_string();
    let staking_pool_account_id = "staking-pool".to_string();

    // Creating whitelist account
    foundation
        .init_whitelist(&mut r, staking_pool_whitelist_account_id.clone())
        .unwrap();

    // Whitelisting staking pool
    foundation
        .function_call(
            &mut r,
            &staking_pool_whitelist_account_id,
            "add_staking_pool",
            &serde_json::to_vec(
                &json!({"staking_pool_account_id": staking_pool_account_id.clone()}),
            )
            .unwrap(),
        )
        .unwrap();

    // Creating staking pool
    foundation
        .init_staking_pool(&mut r, staking_pool_account_id.clone())
        .unwrap();

    // Whitelisting staking pool
    foundation
        .function_call(
            &mut r,
            &staking_pool_whitelist_account_id,
            "add_staking_pool",
            &serde_json::to_vec(
                &json!({"staking_pool_account_id": staking_pool_account_id.clone()}),
            )
            .unwrap(),
        )
        .unwrap();

    let start_timestamp = r.current_block().block_timestamp;

    let vesting_schedule = VestingSchedule {
        start_timestamp: start_timestamp.into(),
        cliff_timestamp: (start_timestamp + 1000).into(),
        end_timestamp: (start_timestamp + 4000).into(),
    };
    let vesting_schedule_str =
        serde_json::to_string(&json!({ "vesting_schedule": vesting_schedule })).unwrap();
    let salt: Vec<u8> = [vec![1, 2, 3], b"VERY_LONG_SALT".to_vec()].concat();
    let args = InitLockupArgs {
        owner_account_id: owner.account_id.clone(),
        lockup_duration: 1000000000.into(),
        lockup_timestamp: None,
        transfers_information: TransfersInformation::TransfersDisabled {
            transfer_poll_account_id: "transfer-poll".to_string(),
        },
        vesting_schedule: Some(VestingScheduleOrHash::VestingHash(
            hash_vesting_schedule(&vesting_schedule, &salt).into(),
        )),
        release_duration: None,
        foundation_account_id: Some(foundation.account_id.clone()),
        staking_pool_whitelist_account_id: staking_pool_whitelist_account_id.clone(),
    };

    foundation
        .init_lockup(&mut r, &args, lockup_amount)
        .unwrap();

    let owner_staking_account = owner.clone();

    let res: Option<AccountId> = call_lockup(&r, "get_staking_pool_account_id", "");
    assert_eq!(res, None);

    // Selecting staking pool
    owner_staking_account
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "select_staking_pool",
            &serde_json::to_vec(
                &json!({"staking_pool_account_id": staking_pool_account_id.clone()}),
            )
            .unwrap(),
        )
        .unwrap();

    let res: Option<AccountId> = call_lockup(&r, "get_staking_pool_account_id", "");
    assert_eq!(res, Some(staking_pool_account_id.clone()));
    let res: U128 = call_lockup(&r, "get_known_deposited_balance", "");
    assert_eq!(res.0, 0);

    // Depositing and staking on the staking pool
    let staking_amount = lockup_amount - ntoy(100);
    owner_staking_account
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "deposit_and_stake",
            &serde_json::to_vec(&json!({ "amount": U128(staking_amount) })).unwrap(),
        )
        .unwrap();

    let res: U128 = call_lockup(&r, "get_known_deposited_balance", "");
    assert_eq!(res.0, staking_amount);

    // Simulating rewards
    foundation
        .transfer(&mut r, &staking_pool_account_id, ntoy(10))
        .unwrap();

    // Pinging the staking pool
    foundation
        .function_call(&mut r, &staking_pool_account_id, "ping", &[])
        .unwrap();

    let res: WrappedBalance =
        call_lockup(&r, "get_locked_vested_amount", vesting_schedule_str.clone());
    assert_eq!(res.0, 0);

    // Updating the timestamp to simulate some vesting
    r.current_block().block_timestamp = start_timestamp + 1500;

    let res: WrappedBalance =
        call_lockup(&r, "get_locked_vested_amount", vesting_schedule_str.clone());
    assert_eq!(res.0, (lockup_amount + ntoy(35)) * 3 / 8);

    let res: WrappedBalance = call_lockup(&r, "get_unvested_amount", vesting_schedule_str.clone());
    assert_eq!(res.0, (lockup_amount + ntoy(35)) * 5 / 8);

    // Terminating the vesting schedule

    let res: Option<TerminationStatus> = call_lockup(&r, "get_termination_status", "");
    assert!(res.is_none());

    let res: WrappedBalance = call_lockup(&r, "get_terminated_unvested_balance", "");
    assert_eq!(res.0, 0);

    // Decrease timestamp by (-1) to make balances round
    r.current_block().block_timestamp = start_timestamp + 1499;

    foundation
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "terminate_vesting",
            &serde_json::to_vec(&json!({
                "vesting_schedule": vesting_schedule,
                "salt": Base64VecU8::from(salt)
            }))
            .unwrap(),
        )
        .unwrap();

    let res: Option<TerminationStatus> = call_lockup(&r, "get_termination_status", "");
    assert_eq!(res, Some(TerminationStatus::VestingTerminatedWithDeficit));

    let res: WrappedBalance = call_lockup(&r, "get_terminated_unvested_balance", "");
    let unvested_balance = (lockup_amount + ntoy(35)) * 5 / 8;
    assert_eq!(res.0, unvested_balance);

    let res: WrappedBalance = call_lockup(&r, "get_terminated_unvested_balance_deficit", "");
    // The rest of the tokens are on the staking pool.
    assert_eq!(res.0, unvested_balance - ntoy(100));

    let res: U128 = call_view(
        &r,
        &staking_pool_account_id.clone(),
        "get_account_staked_balance",
        &serde_json::to_string(&json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })).unwrap(),
    );
    assert!(res.0 > 0);

    foundation
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "termination_prepare_to_withdraw",
            b"{}",
        )
        .unwrap();

    let res: Option<TerminationStatus> = call_lockup(&r, "get_termination_status", "");
    assert_eq!(res, Some(TerminationStatus::EverythingUnstaked));

    let res: WrappedBalance = call_lockup(&r, "get_terminated_unvested_balance_deficit", "");
    assert_eq!(res.0, unvested_balance - ntoy(100));

    let res: U128 = call_view(
        &r,
        &staking_pool_account_id.clone(),
        "get_account_staked_balance",
        &serde_json::to_string(&json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })).unwrap(),
    );
    assert_eq!(res.0, 0);

    let res: U128 = call_view(
        &r,
        &staking_pool_account_id.clone(),
        "get_account_unstaked_balance",
        &serde_json::to_string(&json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })).unwrap(),
    );
    assert!(res.0 > 0);

    let res = foundation
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "termination_prepare_to_withdraw",
            b"{}",
        )
        .unwrap();
    // Need to wait 4 epochs
    assert_eq!(res.status, ExecutionStatus::SuccessValue(b"false".to_vec()));

    let res: Option<TerminationStatus> = call_lockup(&r, "get_termination_status", "");
    assert_eq!(res, Some(TerminationStatus::EverythingUnstaked));

    for _ in 0..4 {
        wait_epoch(&mut r);
    }

    // The standalone runtime doesn't unlock locked balance. Need to manually intervene.
    let mut pool = r.view_account(&staking_pool_account_id).unwrap();
    pool.amount += pool.locked;
    pool.locked = 0;
    r.force_account_update(staking_pool_account_id.clone(), &pool);

    let res: U128 = call_view(
        &r,
        &staking_pool_account_id.clone(),
        "get_account_unstaked_balance",
        &serde_json::to_string(&json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })).unwrap(),
    );
    let received_reward = res.0 - staking_amount;

    let res = foundation
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "termination_prepare_to_withdraw",
            b"{}",
        )
        .unwrap();
    assert_eq!(res.status, ExecutionStatus::SuccessValue(b"true".to_vec()));

    let res: WrappedBalance = call_lockup(&r, "get_terminated_unvested_balance_deficit", "");
    assert_eq!(res.0, 0);

    let res: Option<TerminationStatus> = call_lockup(&r, "get_termination_status", "");
    assert_eq!(res, Some(TerminationStatus::ReadyToWithdraw));

    let res: U128 = call_view(
        &r,
        &staking_pool_account_id.clone(),
        "get_account_unstaked_balance",
        &serde_json::to_string(&json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })).unwrap(),
    );
    assert_eq!(res.0, 0);

    let res: U128 = call_lockup(&r, "get_known_deposited_balance", "");
    assert_eq!(res.0, 0);

    let foundation_balance = foundation.account(&r).amount;

    let res = foundation
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "termination_withdraw",
            &serde_json::to_vec(&json!({ "receiver_id": foundation.account_id.clone() })).unwrap(),
        )
        .unwrap();
    assert_eq!(res.status, ExecutionStatus::SuccessValue(b"true".to_vec()));

    let res: Option<TerminationStatus> = call_lockup(&r, "get_termination_status", "");
    assert_eq!(res, None);

    let res: WrappedBalance = call_lockup(&r, "get_terminated_unvested_balance", "");
    assert_eq!(res.0, 0);

    let new_foundation_balance = foundation.account(&r).amount;
    assert_eq!(
        new_foundation_balance,
        foundation_balance + unvested_balance
    );

    let res: WrappedBalance = call_lockup(&r, "get_locked_amount", "");
    assert_eq!(res.0, (lockup_amount + ntoy(35)) - unvested_balance);

    let res: WrappedBalance = call_lockup(&r, "get_liquid_owners_balance", "");
    assert_eq!(res.0, received_reward);

    let res: WrappedBalance = call_lockup(&r, "get_balance", "");
    assert_eq!(
        res.0,
        (lockup_amount + ntoy(35)) - unvested_balance + received_reward
    );
}

#[test]
fn termination_with_staking() {
    let lockup_amount = ntoy(1000);
    let (mut r, foundation, owner) = basic_setup();

    let staking_pool_whitelist_account_id = "staking-pool-whitelist".to_string();
    let staking_pool_account_id = "staking-pool".to_string();

    // Creating whitelist account
    foundation
        .init_whitelist(&mut r, staking_pool_whitelist_account_id.clone())
        .unwrap();

    // Whitelisting staking pool
    foundation
        .function_call(
            &mut r,
            &staking_pool_whitelist_account_id,
            "add_staking_pool",
            &serde_json::to_vec(
                &json!({"staking_pool_account_id": staking_pool_account_id.clone()}),
            )
            .unwrap(),
        )
        .unwrap();

    // Creating staking pool
    foundation
        .init_staking_pool(&mut r, staking_pool_account_id.clone())
        .unwrap();

    // Whitelisting staking pool
    foundation
        .function_call(
            &mut r,
            &staking_pool_whitelist_account_id,
            "add_staking_pool",
            &serde_json::to_vec(
                &json!({"staking_pool_account_id": staking_pool_account_id.clone()}),
            )
            .unwrap(),
        )
        .unwrap();

    let start_timestamp = r.current_block().block_timestamp;

    let vesting_schedule = VestingSchedule {
        start_timestamp: start_timestamp.into(),
        cliff_timestamp: (start_timestamp + 1000).into(),
        end_timestamp: (start_timestamp + 4000).into(),
    };
    let vesting_schedule_str =
        serde_json::to_string(&json!({ "vesting_schedule": vesting_schedule })).unwrap();
    let args = InitLockupArgs {
        owner_account_id: owner.account_id.clone(),
        lockup_duration: 1000000000.into(),
        lockup_timestamp: None,
        transfers_information: TransfersInformation::TransfersDisabled {
            transfer_poll_account_id: "transfer-poll".to_string(),
        },
        vesting_schedule: Some(VestingScheduleOrHash::VestingSchedule(
            vesting_schedule.clone(),
        )),
        release_duration: None,
        foundation_account_id: Some(foundation.account_id.clone()),
        staking_pool_whitelist_account_id: staking_pool_whitelist_account_id.clone(),
    };

    foundation
        .init_lockup(&mut r, &args, lockup_amount)
        .unwrap();

    let owner_staking_account = owner.clone();

    let res: Option<AccountId> = call_lockup(&r, "get_staking_pool_account_id", "");
    assert_eq!(res, None);

    // Selecting staking pool
    owner_staking_account
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "select_staking_pool",
            &serde_json::to_vec(
                &json!({"staking_pool_account_id": staking_pool_account_id.clone()}),
            )
            .unwrap(),
        )
        .unwrap();

    let res: Option<AccountId> = call_lockup(&r, "get_staking_pool_account_id", "");
    assert_eq!(res, Some(staking_pool_account_id.clone()));
    let res: U128 = call_lockup(&r, "get_known_deposited_balance", "");
    assert_eq!(res.0, 0);

    // Depositing and staking on the staking pool
    let staking_amount = lockup_amount - ntoy(100);
    owner_staking_account
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "deposit_and_stake",
            &serde_json::to_vec(&json!({ "amount": U128(staking_amount) })).unwrap(),
        )
        .unwrap();

    let res: U128 = call_lockup(&r, "get_known_deposited_balance", "");
    assert_eq!(res.0, staking_amount);

    // Simulating rewards
    foundation
        .transfer(&mut r, &staking_pool_account_id, ntoy(10))
        .unwrap();

    // Pinging the staking pool
    foundation
        .function_call(&mut r, &staking_pool_account_id, "ping", &[])
        .unwrap();

    let res: WrappedBalance =
        call_lockup(&r, "get_locked_vested_amount", vesting_schedule_str.clone());
    assert_eq!(res.0, 0);

    // Updating the timestamp to simulate some vesting
    r.current_block().block_timestamp = start_timestamp + 1500;

    let res: WrappedBalance =
        call_lockup(&r, "get_locked_vested_amount", vesting_schedule_str.clone());
    assert_eq!(res.0, (lockup_amount + ntoy(35)) * 3 / 8);

    let res: WrappedBalance = call_lockup(&r, "get_unvested_amount", vesting_schedule_str.clone());
    assert_eq!(res.0, (lockup_amount + ntoy(35)) * 5 / 8);

    // Terminating the vesting schedule

    let res: Option<TerminationStatus> = call_lockup(&r, "get_termination_status", "");
    assert!(res.is_none());

    let res: WrappedBalance = call_lockup(&r, "get_terminated_unvested_balance", "");
    assert_eq!(res.0, 0);

    // Decrease timestamp by (-1) to make balances round
    r.current_block().block_timestamp = start_timestamp + 1499;

    foundation
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "terminate_vesting",
            &serde_json::to_vec(&json!({
                "vesting_schedule": vesting_schedule,
                "salt": Base64VecU8::from(vec![])
            }))
            .unwrap(),
        )
        .unwrap();

    let res: Option<TerminationStatus> = call_lockup(&r, "get_termination_status", "");
    assert_eq!(res, Some(TerminationStatus::VestingTerminatedWithDeficit));

    let res: WrappedBalance = call_lockup(&r, "get_terminated_unvested_balance", "");
    let unvested_balance = (lockup_amount + ntoy(35)) * 5 / 8;
    assert_eq!(res.0, unvested_balance);

    let res: WrappedBalance = call_lockup(&r, "get_terminated_unvested_balance_deficit", "");
    // The rest of the tokens are on the staking pool.
    assert_eq!(res.0, unvested_balance - ntoy(100));

    let res: U128 = call_view(
        &r,
        &staking_pool_account_id.clone(),
        "get_account_staked_balance",
        &serde_json::to_string(&json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })).unwrap(),
    );
    assert!(res.0 > 0);

    foundation
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "termination_prepare_to_withdraw",
            b"{}",
        )
        .unwrap();

    let res: Option<TerminationStatus> = call_lockup(&r, "get_termination_status", "");
    assert_eq!(res, Some(TerminationStatus::EverythingUnstaked));

    let res: WrappedBalance = call_lockup(&r, "get_terminated_unvested_balance_deficit", "");
    assert_eq!(res.0, unvested_balance - ntoy(100));

    let res: U128 = call_view(
        &r,
        &staking_pool_account_id.clone(),
        "get_account_staked_balance",
        &serde_json::to_string(&json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })).unwrap(),
    );
    assert_eq!(res.0, 0);

    let res: U128 = call_view(
        &r,
        &staking_pool_account_id.clone(),
        "get_account_unstaked_balance",
        &serde_json::to_string(&json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })).unwrap(),
    );
    assert!(res.0 > 0);

    let res = foundation
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "termination_prepare_to_withdraw",
            b"{}",
        )
        .unwrap();
    // Need to wait 4 epochs
    assert_eq!(res.status, ExecutionStatus::SuccessValue(b"false".to_vec()));

    let res: Option<TerminationStatus> = call_lockup(&r, "get_termination_status", "");
    assert_eq!(res, Some(TerminationStatus::EverythingUnstaked));

    for _ in 0..4 {
        wait_epoch(&mut r);
    }

    // The standalone runtime doesn't unlock locked balance. Need to manually intervene.
    let mut pool = r.view_account(&staking_pool_account_id).unwrap();
    pool.amount += pool.locked;
    pool.locked = 0;
    r.force_account_update(staking_pool_account_id.clone(), &pool);

    let res: U128 = call_view(
        &r,
        &staking_pool_account_id.clone(),
        "get_account_unstaked_balance",
        &serde_json::to_string(&json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })).unwrap(),
    );
    let received_reward = res.0 - staking_amount;

    let res = foundation
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "termination_prepare_to_withdraw",
            b"{}",
        )
        .unwrap();
    assert_eq!(res.status, ExecutionStatus::SuccessValue(b"true".to_vec()));

    let res: WrappedBalance = call_lockup(&r, "get_terminated_unvested_balance_deficit", "");
    assert_eq!(res.0, 0);

    let res: Option<TerminationStatus> = call_lockup(&r, "get_termination_status", "");
    assert_eq!(res, Some(TerminationStatus::ReadyToWithdraw));

    let res: U128 = call_view(
        &r,
        &staking_pool_account_id.clone(),
        "get_account_unstaked_balance",
        &serde_json::to_string(&json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })).unwrap(),
    );
    assert_eq!(res.0, 0);

    let res: U128 = call_lockup(&r, "get_known_deposited_balance", "");
    assert_eq!(res.0, 0);

    let foundation_balance = foundation.account(&r).amount;

    let res = foundation
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "termination_withdraw",
            &serde_json::to_vec(&json!({ "receiver_id": foundation.account_id.clone() })).unwrap(),
        )
        .unwrap();
    assert_eq!(res.status, ExecutionStatus::SuccessValue(b"true".to_vec()));

    let res: Option<TerminationStatus> = call_lockup(&r, "get_termination_status", "");
    assert_eq!(res, None);

    let res: WrappedBalance = call_lockup(&r, "get_terminated_unvested_balance", "");
    assert_eq!(res.0, 0);

    let new_foundation_balance = foundation.account(&r).amount;
    assert_eq!(
        new_foundation_balance,
        foundation_balance + unvested_balance
    );

    let res: WrappedBalance = call_lockup(&r, "get_locked_amount", "");
    assert_eq!(res.0, (lockup_amount + ntoy(35)) - unvested_balance);

    let res: WrappedBalance = call_lockup(&r, "get_liquid_owners_balance", "");
    assert_eq!(res.0, received_reward);

    let res: WrappedBalance = call_lockup(&r, "get_balance", "");
    assert_eq!(
        res.0,
        (lockup_amount + ntoy(35)) - unvested_balance + received_reward
    );
}

#[test]
fn test_release_schedule_unlock_transfers() {
    let lockup_amount = ntoy(1000);
    let (mut r, foundation, owner) = basic_setup();

    let staking_pool_whitelist_account_id = "staking-pool-whitelist".to_string();
    let staking_pool_account_id = "staking-pool".to_string();
    let transfer_poll_account_id = "transfer-poll".to_string();

    // Initializing fake voting contract
    foundation
        .init_transfer_poll(&mut r, transfer_poll_account_id.clone())
        .unwrap();

    // Creating whitelist account
    foundation
        .init_whitelist(&mut r, staking_pool_whitelist_account_id.clone())
        .unwrap();

    // Whitelisting staking pool
    foundation
        .function_call(
            &mut r,
            &staking_pool_whitelist_account_id,
            "add_staking_pool",
            &serde_json::to_vec(
                &json!({"staking_pool_account_id": staking_pool_account_id.clone()}),
            )
            .unwrap(),
        )
        .unwrap();

    // Creating staking pool
    foundation
        .init_staking_pool(&mut r, staking_pool_account_id.clone())
        .unwrap();

    // Whitelisting staking pool
    foundation
        .function_call(
            &mut r,
            &staking_pool_whitelist_account_id,
            "add_staking_pool",
            &serde_json::to_vec(
                &json!({"staking_pool_account_id": staking_pool_account_id.clone()}),
            )
            .unwrap(),
        )
        .unwrap();

    // Unlock timestamp from fake voting contract.
    let unlock_timestamp = 1535760000000000000u64;
    r.current_block().block_timestamp = unlock_timestamp + 1000;

    let args = InitLockupArgs {
        owner_account_id: owner.account_id.clone(),
        lockup_duration: 1000000000.into(),
        lockup_timestamp: None,
        transfers_information: TransfersInformation::TransfersDisabled {
            transfer_poll_account_id,
        },
        vesting_schedule: None,
        release_duration: Some(1000000000000.into()),
        foundation_account_id: None,
        staking_pool_whitelist_account_id: staking_pool_whitelist_account_id.clone(),
    };

    foundation
        .init_lockup(&mut r, &args, lockup_amount)
        .unwrap();

    let owner_staking_account = owner.clone();

    let res: Option<AccountId> = call_lockup(&r, "get_staking_pool_account_id", "");
    assert_eq!(res, None);

    // Selecting staking pool
    owner_staking_account
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "select_staking_pool",
            &serde_json::to_vec(
                &json!({"staking_pool_account_id": staking_pool_account_id.clone()}),
            )
            .unwrap(),
        )
        .unwrap();

    let res: Option<AccountId> = call_lockup(&r, "get_staking_pool_account_id", "");
    assert_eq!(res, Some(staking_pool_account_id.clone()));
    let res: U128 = call_lockup(&r, "get_known_deposited_balance", "");
    assert_eq!(res.0, 0);

    // Depositing and staking on the staking pool
    let staking_amount = lockup_amount - ntoy(100);
    owner_staking_account
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "deposit_and_stake",
            &serde_json::to_vec(&json!({ "amount": U128(staking_amount) })).unwrap(),
        )
        .unwrap();

    let res: U128 = call_lockup(&r, "get_known_deposited_balance", "");
    assert_eq!(res.0, staking_amount);

    // Simulating rewards
    foundation
        .transfer(&mut r, &staking_pool_account_id, ntoy(10))
        .unwrap();

    // Pinging the staking pool
    foundation
        .function_call(&mut r, &staking_pool_account_id, "ping", &[])
        .unwrap();

    let res: U128 = call_view(
        &r,
        &staking_pool_account_id.clone(),
        "get_account_staked_balance",
        &serde_json::to_string(&json!({ "account_id": LOCKUP_ACCOUNT_ID.to_string() })).unwrap(),
    );
    let received_reward = res.0 - staking_amount;

    owner_staking_account
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "refresh_staking_pool_balance",
            &[],
        )
        .unwrap();

    let full_lockup_amount = lockup_amount + ntoy(35);

    let res: U128 = call_lockup(&r, "get_locked_amount", "");
    assert_eq!(res.0, full_lockup_amount);

    let res: U128 = call_lockup(&r, "get_known_deposited_balance", "");
    assert_eq!(res.0, staking_amount + received_reward);

    let res: bool = call_lockup(&r, "are_transfers_enabled", "");
    assert!(!res);

    let res: U128 = call_lockup(&r, "get_liquid_owners_balance", "");
    assert_eq!(res.0, received_reward);

    let res: U128 = call_lockup(&r, "get_owners_balance", "");
    assert_eq!(res.0, received_reward);

    let res: U128 = call_lockup(&r, "get_balance", "");
    assert_eq!(res.0, full_lockup_amount + received_reward);

    let transfer_amount = ntoy(5);
    assert!(transfer_amount < received_reward);
    let owner_balance = owner.account(&r).amount;

    owner_staking_account
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "transfer",
            &serde_json::to_vec(&json!({ "amount": U128(transfer_amount), "receiver_id": owner.account_id.clone() })).unwrap(),
        )
        .expect_err("expected failure because transfers are disabled");

    let new_owner_balance = owner.account(&r).amount;
    assert_eq!(new_owner_balance, owner_balance);

    let res = owner_staking_account
        .function_call(&mut r, LOCKUP_ACCOUNT_ID, "check_transfers_vote", &[])
        .unwrap();
    assert_eq!(res.status, ExecutionStatus::SuccessValue(b"true".to_vec()));

    let res: U128 = call_lockup(&r, "get_locked_amount", "");
    assert_eq!(res.0, lockup_amount + ntoy(35));

    let res: bool = call_lockup(&r, "are_transfers_enabled", "");
    assert!(res);

    let res: U128 = call_lockup(&r, "get_liquid_owners_balance", "");
    assert_eq!(res.0, received_reward);

    let res: U128 = call_lockup(&r, "get_owners_balance", "");
    assert_eq!(res.0, received_reward);

    let res: U128 = call_lockup(&r, "get_balance", "");
    assert_eq!(res.0, full_lockup_amount + received_reward);

    owner_staking_account
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "transfer",
            &serde_json::to_vec(&json!({ "amount": U128(transfer_amount), "receiver_id": owner.account_id.clone() })).unwrap(),
        )
        .unwrap();

    let new_owner_balance = owner.account(&r).amount;
    assert_eq!(new_owner_balance, owner_balance + transfer_amount);

    let liquid_balance = received_reward - transfer_amount;

    let res: U128 = call_lockup(&r, "get_locked_amount", "");
    assert_eq!(res.0, full_lockup_amount);

    let res: U128 = call_lockup(&r, "get_liquid_owners_balance", "");
    assert_eq!(res.0, liquid_balance);

    let res: U128 = call_lockup(&r, "get_owners_balance", "");
    assert_eq!(res.0, liquid_balance);

    let res: U128 = call_lockup(&r, "get_balance", "");
    assert_eq!(res.0, full_lockup_amount + liquid_balance);

    let transfer_amount = ntoy(15);
    assert!(transfer_amount > liquid_balance);

    let owner_balance = new_owner_balance;

    owner_staking_account
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "transfer",
            &serde_json::to_vec(&json!({ "amount": U128(transfer_amount), "receiver_id": owner.account_id.clone() })).unwrap(),
        )
        .expect_err("expected failure because not enough liquid balance");

    let new_owner_balance = owner.account(&r).amount;
    assert_eq!(new_owner_balance, owner_balance);

    // Adjusting block timestamp to be after lockup duration.
    // At this timestamp only 1/1000 of the lockup_amount is released.
    r.current_block().block_timestamp = unlock_timestamp + 1000000000;

    let res: U128 = call_lockup(&r, "get_locked_amount", "");
    assert_eq!(res.0, full_lockup_amount - full_lockup_amount / 1000);

    let res: U128 = call_lockup(&r, "get_liquid_owners_balance", "");
    assert_eq!(res.0, liquid_balance + full_lockup_amount / 1000);

    let res: U128 = call_lockup(&r, "get_owners_balance", "");
    assert_eq!(res.0, liquid_balance + full_lockup_amount / 1000);

    let res: U128 = call_lockup(&r, "get_balance", "");
    assert_eq!(res.0, full_lockup_amount + liquid_balance);

    // Adding more time. So 50/1000 is unlocked
    r.current_block().block_timestamp = unlock_timestamp + 50_000_000_000;

    let res: U128 = call_lockup(&r, "get_locked_amount", "");
    assert_eq!(res.0, full_lockup_amount - 50 * full_lockup_amount / 1000);

    let res: U128 = call_lockup(&r, "get_liquid_owners_balance", "");
    assert_eq!(res.0, liquid_balance + 50 * full_lockup_amount / 1000);

    let res: U128 = call_lockup(&r, "get_owners_balance", "");
    assert_eq!(res.0, liquid_balance + 50 * full_lockup_amount / 1000);

    let res: U128 = call_lockup(&r, "get_balance", "");
    assert_eq!(res.0, full_lockup_amount + liquid_balance);

    // Transferring 15 more
    owner_staking_account
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "transfer",
            &serde_json::to_vec(&json!({ "amount": U128(transfer_amount), "receiver_id": owner.account_id.clone() })).unwrap(),
        )
        .unwrap();

    let new_owner_balance = owner.account(&r).amount;
    assert_eq!(new_owner_balance, owner_balance + transfer_amount);

    let full_balance = full_lockup_amount + liquid_balance - transfer_amount;
    let liquid_balance = liquid_balance + 51 * full_lockup_amount / 1000 - transfer_amount;

    // Setting time to 51/1000 to have round numbers
    r.current_block().block_timestamp = unlock_timestamp + 51_000_000_000;

    let res: U128 = call_lockup(&r, "get_locked_amount", "");
    assert_eq!(res.0, full_lockup_amount - 51 * full_lockup_amount / 1000);

    let res: U128 = call_lockup(&r, "get_liquid_owners_balance", "");
    assert_eq!(res.0, liquid_balance);

    let res: U128 = call_lockup(&r, "get_owners_balance", "");
    assert_eq!(res.0, liquid_balance);

    let res: U128 = call_lockup(&r, "get_balance", "");
    assert_eq!(res.0, full_balance);

    // Setting time to 200/1000 to check liquid balance, because majority of balance is still staked.
    r.current_block().block_timestamp = unlock_timestamp + 200_000_000_000;
    let owners_balance = liquid_balance + 149 * full_lockup_amount / 1000;

    let res: U128 = call_lockup(&r, "get_locked_amount", "");
    assert_eq!(res.0, full_lockup_amount - 200 * full_lockup_amount / 1000);

    let res: U128 = call_lockup(&r, "get_liquid_owners_balance", "");
    // The account balance is `100`. `+35` for storage and `-20` for transfers.
    assert_eq!(res.0, ntoy(80));

    let res: U128 = call_lockup(&r, "get_owners_balance", "");
    assert_eq!(res.0, owners_balance);

    let res: U128 = call_lockup(&r, "get_balance", "");
    assert_eq!(res.0, full_balance);

    let public_key: Base58PublicKey = owner_staking_account
        .signer()
        .public_key
        .try_to_vec()
        .unwrap()
        .try_into()
        .unwrap();

    // Trying to add full access key.
    owner_staking_account
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "add_full_access_key",
            &serde_json::to_vec(&json!({ "new_public_key": public_key.clone() })).unwrap(),
        )
        .expect_err("not fully unlocked");

    // Setting time to full release.
    r.current_block().block_timestamp = unlock_timestamp + 1100_000_000_000;

    let res: U128 = call_lockup(&r, "get_locked_amount", "");
    assert_eq!(res.0, 0);

    let res: U128 = call_lockup(&r, "get_liquid_owners_balance", "");
    assert_eq!(res.0, ntoy(80));

    let res: U128 = call_lockup(&r, "get_owners_balance", "");
    assert_eq!(res.0, full_balance);

    let res: U128 = call_lockup(&r, "get_balance", "");
    assert_eq!(res.0, full_balance);

    // Adding full access key
    owner_staking_account
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "add_full_access_key",
            &serde_json::to_vec(&json!({ "new_public_key": public_key.clone() })).unwrap(),
        )
        .unwrap();

    let mut lockup_account = owner_staking_account.clone();
    lockup_account.account_id = LOCKUP_ACCOUNT_ID.to_string();

    // Testing direct transfer
    let owner_balance = new_owner_balance;
    lockup_account
        .transfer(&mut r, &owner.account_id, transfer_amount)
        .unwrap();

    let new_owner_balance = owner.account(&r).amount;
    assert_eq!(new_owner_balance, owner_balance + transfer_amount);
}

fn basic_setup() -> (RuntimeStandalone, ExternalUser, ExternalUser) {
    let (mut r, foundation) = new_root("foundation".into());

    let owner = foundation
        .create_external(&mut r, "owner".into(), ntoy(30))
        .unwrap();
    (r, foundation, owner)
}
