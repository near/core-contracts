mod utils;

use crate::utils::{call_view, new_root, ntoy, view_factory, ExternalUser, FACTORY_ACCOUNT_ID};
use borsh::BorshSerialize;
use near_runtime_standalone::RuntimeStandalone;
use near_sdk::json_types::{Base58PublicKey, U128};
use serde_json::json;
use std::convert::TryInto;

#[test]
fn create_staking_pool() {
    let (mut r, foundation, owner) = basic_setup();

    let staking_pool_whitelist_account_id = "staking-pool-whitelist".to_string();
    let staking_pool_id = "pool".to_string();
    let staking_pool_account_id = "pool.factory".to_string();
    let owner_staking_account_id = "owner-staking".to_string();

    // Creating whitelist account
    foundation
        .init_whitelist(&mut r, staking_pool_whitelist_account_id.clone())
        .unwrap();

    let is_pool_whitelisted: bool = call_view(
        &r,
        &staking_pool_whitelist_account_id,
        "is_whitelisted",
        &serde_json::to_string(
            &json!({ "staking_pool_account_id": staking_pool_account_id.clone() }),
        )
        .unwrap(),
    );
    assert!(!is_pool_whitelisted);

    let is_factory_whitelisted: bool = call_view(
        &r,
        &staking_pool_whitelist_account_id,
        "is_factory_whitelisted",
        &serde_json::to_string(&json!({ "factory_account_id": FACTORY_ACCOUNT_ID.to_string() }))
            .unwrap(),
    );
    assert!(!is_factory_whitelisted);

    // Whitelisting staking pool
    foundation
        .function_call(
            &mut r,
            &staking_pool_whitelist_account_id.clone(),
            "add_factory",
            &serde_json::to_vec(&json!({"factory_account_id": FACTORY_ACCOUNT_ID.to_string()}))
                .unwrap(),
            0,
        )
        .unwrap();

    let is_pool_whitelisted: bool = call_view(
        &r,
        &staking_pool_whitelist_account_id,
        "is_whitelisted",
        &serde_json::to_string(
            &json!({ "staking_pool_account_id": staking_pool_account_id.clone() }),
        )
        .unwrap(),
    );
    assert!(!is_pool_whitelisted);

    let is_factory_whitelisted: bool = call_view(
        &r,
        &staking_pool_whitelist_account_id,
        "is_factory_whitelisted",
        &serde_json::to_string(&json!({ "factory_account_id": FACTORY_ACCOUNT_ID.to_string() }))
            .unwrap(),
    );
    assert!(is_factory_whitelisted);

    // Creating staking pool
    foundation
        .init_factory(&mut r, &staking_pool_whitelist_account_id)
        .unwrap();

    let res: U128 = view_factory(&r, "get_min_attached_balance", "");
    assert_eq!(res.0, ntoy(30));

    let res: u64 = view_factory(&r, "get_number_of_staking_pools_created", "");
    assert_eq!(res, 0);

    // Add new access key for calling staking methods
    let owner_staking_account = foundation
        .create_external(&mut r, owner_staking_account_id.clone(), ntoy(30))
        .unwrap();
    let staking_key: Base58PublicKey = owner_staking_account
        .signer()
        .public_key
        .try_to_vec()
        .unwrap()
        .try_into()
        .unwrap();

    owner
        .function_call(
            &mut r,
            FACTORY_ACCOUNT_ID,
            "create_staking_pool",
            &serde_json::to_vec(&json!({
                "staking_pool_id": staking_pool_id.clone(),
                "owner_id": owner_staking_account_id.clone(),
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

    let res: u64 = view_factory(&r, "get_number_of_staking_pools_created", "");
    assert_eq!(res, 1);

    let is_whitelisted: bool = call_view(
        &r,
        &staking_pool_whitelist_account_id,
        "is_whitelisted",
        &serde_json::to_string(
            &json!({ "staking_pool_account_id": staking_pool_account_id.clone() }),
        )
        .unwrap(),
    );
    assert!(is_whitelisted);

    let actual_staking_key: Base58PublicKey =
        call_view(&r, &staking_pool_account_id, "get_staking_key", "");
    assert_eq!(actual_staking_key.0, staking_key.0);
}

fn basic_setup() -> (RuntimeStandalone, ExternalUser, ExternalUser) {
    let (mut r, foundation) = new_root("foundation".into());

    let owner = foundation
        .create_external(&mut r, "owner".into(), ntoy(100))
        .unwrap();

    (r, foundation, owner)
}
