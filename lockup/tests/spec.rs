extern crate quickcheck;
#[macro_use(quickcheck)]
extern crate quickcheck_macros;

mod utils;

use crate::utils::{call_view, wait_epoch, ExternalUser, LOCKUP_ACCOUNT_ID};
use borsh::BorshSerialize;
use lockup_contract::LockupStartInformation;
use near_primitives::transaction::ExecutionStatus;
use near_primitives::types::Balance;
use near_runtime_standalone::RuntimeStandalone;
use near_sdk::json_types::{Base58PublicKey, U128};
use near_sdk::AccountId;
use serde_json::json;
use std::convert::TryInto;
use utils::{call_lockup, new_root, ntoy, InitLockupArgs};

#[quickcheck]
fn lockup(lockup_amount: Balance, lockup_duration: u64, lockup_timestamp: u64) {
    let (mut r, foundation, initial_owners_main_public_key, _) = basic_setup();

    let args = InitLockupArgs {
        lockup_duration: lockup_duration.into(),
        lockup_start_information: LockupStartInformation::TransfersEnabled {
            lockup_timestamp: lockup_timestamp.saturating_add(1).into(),
        },
        initial_owners_main_public_key,
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
    let (mut r, foundation, initial_owners_main_public_key, owner) = basic_setup();

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
        lockup_duration: 1000000000.into(),
        lockup_start_information: LockupStartInformation::TransfersDisabled {
            transfer_poll_account_id: "transfer-poll".to_string(),
        },
        initial_owners_main_public_key,
        foundation_account_id: None,
        staking_pool_whitelist_account_id: staking_pool_whitelist_account_id.clone(),
    };

    foundation
        .init_lockup(&mut r, &args, lockup_amount)
        .unwrap();

    // Add new access key for calling staking methods
    let mut owner_staking_account = foundation
        .create_external(&mut r, "owner-staking".into(), ntoy(30))
        .unwrap();
    owner_staking_account.account_id = LOCKUP_ACCOUNT_ID.into();
    let staking_access_key: Base58PublicKey = owner_staking_account
        .signer()
        .public_key
        .try_to_vec()
        .unwrap()
        .try_into()
        .unwrap();

    owner
        .function_call(
            &mut r,
            LOCKUP_ACCOUNT_ID,
            "add_staking_access_key",
            &serde_json::to_vec(&json!({"new_public_key": staking_access_key.clone()})).unwrap(),
        )
        .unwrap();

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

fn basic_setup() -> (
    RuntimeStandalone,
    ExternalUser,
    Base58PublicKey,
    ExternalUser,
) {
    let (mut r, foundation) = new_root("foundation".into());

    let mut owner = foundation
        .create_external(&mut r, "owner".into(), ntoy(30))
        .unwrap();

    owner.account_id = LOCKUP_ACCOUNT_ID.into();

    (
        r,
        foundation,
        owner
            .signer()
            .public_key
            .try_to_vec()
            .unwrap()
            .try_into()
            .unwrap(),
        owner,
    )
}
