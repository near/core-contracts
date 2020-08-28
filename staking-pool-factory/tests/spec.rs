mod utils;

use crate::utils::{call_view, new_root, ntoy, view_factory, ExternalUser, FACTORY_ACCOUNT_ID};
use near_primitives::transaction::ExecutionStatus;
use near_runtime_standalone::RuntimeStandalone;
use near_sdk::borsh::BorshSerialize;
use near_sdk::json_types::{Base58PublicKey, U128};
use near_sdk::serde_json::{self, json};
use std::convert::TryInto;

const STAKING_POOL_WHITELIST_ACCOUNT_ID: &str = "staking-pool-whitelist";
const STAKING_POOL_ID: &str = "pool";
const STAKING_POOL_ACCOUNT_ID: &str = "pool.factory";
const OWNER_STAKING_ACCOUNT_ID: &str = "owner-staking";

#[test]
fn create_staking_pool_success() {
    let (mut r, foundation, owner) = setup_factory();

    let res: U128 = view_factory(&r, "get_min_attached_balance", "");
    assert_eq!(res.0, ntoy(30));

    let res: u64 = view_factory(&r, "get_number_of_staking_pools_created", "");
    assert_eq!(res, 0);

    let owner_staking_account = foundation
        .create_external(&mut r, OWNER_STAKING_ACCOUNT_ID.to_string(), ntoy(30))
        .unwrap();
    let staking_key: Base58PublicKey = owner_staking_account
        .signer()
        .public_key
        .try_to_vec()
        .unwrap()
        .try_into()
        .unwrap();

    let owner_balance = owner.account(&r).amount;

    owner
        .function_call(
            &mut r,
            FACTORY_ACCOUNT_ID,
            "create_staking_pool",
            &serde_json::to_vec(&json!({
                "staking_pool_id": STAKING_POOL_ID.to_string(),
                "owner_id": OWNER_STAKING_ACCOUNT_ID.to_string(),
                "stake_public_key": staking_key.clone(),
                "reward_fee_fraction": {
                    "numerator": 10,
                    "denominator": 100,
                }
            }))
            .unwrap(),
            ntoy(31),
        )
        .unwrap();

    // The factory remembered the pool
    let res: u64 = view_factory(&r, "get_number_of_staking_pools_created", "");
    assert_eq!(res, 1);

    // The pool was whitelisted
    let is_whitelisted: bool = call_view(
        &r,
        &STAKING_POOL_WHITELIST_ACCOUNT_ID,
        "is_whitelisted",
        &serde_json::to_string(
            &json!({ "staking_pool_account_id": STAKING_POOL_ACCOUNT_ID.to_string() }),
        )
        .unwrap(),
    );
    assert!(is_whitelisted);

    // The owner was charged the amount
    let new_owner_balance = owner.account(&r).amount;
    assert_eq!(new_owner_balance, owner_balance - ntoy(31));

    // Pool account was created and attached deposit was transferred.
    let pool_account = r.view_account(&STAKING_POOL_ACCOUNT_ID.to_string());
    assert!(pool_account.is_some());
    let pool_account = pool_account.unwrap();
    assert_eq!(pool_account.amount + pool_account.locked, ntoy(31));

    // The staking key on the pool matches the one that was given.
    let actual_staking_key: Base58PublicKey =
        call_view(&r, &STAKING_POOL_ACCOUNT_ID, "get_staking_key", "");
    assert_eq!(actual_staking_key.0, staking_key.0);
}

#[test]
fn create_staking_pool_bad_staking_key() {
    let (mut r, foundation, owner) = setup_factory();

    let res: U128 = view_factory(&r, "get_min_attached_balance", "");
    assert_eq!(res.0, ntoy(30));

    let res: u64 = view_factory(&r, "get_number_of_staking_pools_created", "");
    assert_eq!(res, 0);

    let _owner_staking_account = foundation
        .create_external(&mut r, OWNER_STAKING_ACCOUNT_ID.to_string(), ntoy(30))
        .unwrap();
    let bad_staking_key: Base58PublicKey = vec![0; 33].try_into().unwrap();

    let owner_balance = owner.account(&r).amount;

    let res = owner
        .function_call(
            &mut r,
            FACTORY_ACCOUNT_ID,
            "create_staking_pool",
            &serde_json::to_vec(&json!({
                "staking_pool_id": STAKING_POOL_ID.to_string(),
                "owner_id": OWNER_STAKING_ACCOUNT_ID.to_string(),
                "stake_public_key": bad_staking_key.clone(),
                "reward_fee_fraction": {
                    "numerator": 10,
                    "denominator": 100,
                }
            }))
            .unwrap(),
            ntoy(31),
        )
        .unwrap();
    assert_eq!(res.status, ExecutionStatus::SuccessValue(b"false".to_vec()));

    // Check the factory didn't store the pool.
    let res: u64 = view_factory(&r, "get_number_of_staking_pools_created", "");
    assert_eq!(res, 0);

    // Check the pool was not whitelisted.
    let is_whitelisted: bool = call_view(
        &r,
        &STAKING_POOL_WHITELIST_ACCOUNT_ID,
        "is_whitelisted",
        &serde_json::to_string(
            &json!({ "staking_pool_account_id": STAKING_POOL_ACCOUNT_ID.to_string() }),
        )
        .unwrap(),
    );
    assert!(!is_whitelisted);

    // Check the amount was refunded
    let new_owner_balance = owner.account(&r).amount;
    assert_eq!(new_owner_balance, owner_balance);

    // Pool account was not created
    let pool_account = r.view_account(&STAKING_POOL_ACCOUNT_ID.to_string());
    assert_eq!(pool_account, None);
}

fn setup_factory() -> (RuntimeStandalone, ExternalUser, ExternalUser) {
    let (mut r, foundation) = new_root("foundation".into());

    let owner = foundation
        .create_external(&mut r, "owner".into(), ntoy(100))
        .unwrap();

    // Creating whitelist account
    foundation
        .init_whitelist(&mut r, STAKING_POOL_WHITELIST_ACCOUNT_ID.to_string())
        .unwrap();
    let is_pool_whitelisted: bool = call_view(
        &r,
        &STAKING_POOL_WHITELIST_ACCOUNT_ID,
        "is_whitelisted",
        &serde_json::to_string(
            &json!({ "staking_pool_account_id": STAKING_POOL_ACCOUNT_ID.to_string() }),
        )
        .unwrap(),
    );
    assert!(!is_pool_whitelisted);
    let is_factory_whitelisted: bool = call_view(
        &r,
        &STAKING_POOL_WHITELIST_ACCOUNT_ID,
        "is_factory_whitelisted",
        &serde_json::to_string(&json!({ "factory_account_id": FACTORY_ACCOUNT_ID.to_string() }))
            .unwrap(),
    );
    assert!(!is_factory_whitelisted);
    // Whitelisting staking pool
    foundation
        .function_call(
            &mut r,
            &STAKING_POOL_WHITELIST_ACCOUNT_ID,
            "add_factory",
            &serde_json::to_vec(&json!({"factory_account_id": FACTORY_ACCOUNT_ID.to_string()}))
                .unwrap(),
            0,
        )
        .unwrap();
    let is_pool_whitelisted: bool = call_view(
        &r,
        &STAKING_POOL_WHITELIST_ACCOUNT_ID,
        "is_whitelisted",
        &serde_json::to_string(
            &json!({ "staking_pool_account_id": STAKING_POOL_ACCOUNT_ID.to_string() }),
        )
        .unwrap(),
    );
    assert!(!is_pool_whitelisted);
    let is_factory_whitelisted: bool = call_view(
        &r,
        &STAKING_POOL_WHITELIST_ACCOUNT_ID,
        "is_factory_whitelisted",
        &serde_json::to_string(&json!({ "factory_account_id": FACTORY_ACCOUNT_ID.to_string() }))
            .unwrap(),
    );
    assert!(is_factory_whitelisted);
    // Creating staking pool
    foundation
        .init_factory(&mut r, &STAKING_POOL_WHITELIST_ACCOUNT_ID)
        .unwrap();
    (r, foundation, owner)
}
