mod utils;

use near_primitives::{
    account::AccessKey,
    hash::CryptoHash,
    transaction::{ExecutionOutcome, Transaction},
    types::{AccountId, Balance},
};
use near_sdk::json_types::U128;
use serde::de::DeserializeOwned;
use serde_json::json;
use staking_pool::RewardFeeFraction;
use utils::{ntoy, ExternalUser, POOL_ACCOUNT_ID};

use near_runtime_standalone::{init_runtime_and_signer, RuntimeStandalone};

fn call_view<I: ToString, O: DeserializeOwned>(
    runtime: &mut RuntimeStandalone,
    account_id: &AccountId,
    method: &str,
    args: I,
) -> O {
    let args = args.to_string();
    let result = runtime
        .view_method_call(account_id, method, args.as_bytes())
        .unwrap()
        .0;
    let output: O = serde_json::from_reader(result.as_slice()).unwrap();
    output
}

fn call_pool<I: ToString, O: DeserializeOwned>(
    runtime: &mut RuntimeStandalone,
    method: &str,
    args: I,
) -> O {
    call_view(runtime, &POOL_ACCOUNT_ID.into(), method, args)
}

fn check_invariants(runtime: &mut RuntimeStandalone, users: &[AccountId]) {}

#[test]
fn should_stake() {
    let (mut runtime, signer) = init_runtime_and_signer(&"root".into());
    let root = ExternalUser::new("root".into(), signer);
    let bob = root.create_external(&mut runtime, "bob".into(), ntoy(100));

    let res = root.pool_init_new(
        &mut runtime,
        23 * 10u128.pow(24),
        RewardFeeFraction {
            numerator: 10,
            denominator: 100,
        },
    );
    // println!("{:?}", res);
    dbg!(root.pool_ping(&mut runtime));
    bob.pool_deposit(&mut runtime, 1000);
    bob.pool_stake(&mut runtime, 1000.into());
    let bob_stake: U128 = call_pool(
        &mut runtime,
        "get_account_staked_balance",
        json!({"account_id": "bob"}),
    );

    assert_eq!(bob_stake, 1000.into());

    root.pool_deposit(&mut runtime, 10000);
    root.pool_stake(&mut runtime, 10000.into());
    // wait_epoch(&mut runtime);
    // reward_pool(&mut runtime, 1000);
    // bob.pool_unstake(&mut runtime);
    // let alice = root.create_external(&mut runtime, "alice".into(), ntoy(100));

    // alice.
    // alice.pool_deposit(&mut runtime, 10000);

    // alice.pool_stake(&mut runtime, 1.into());
}

fn reward_pool(runtime: &mut RuntimeStandalone, amount: Balance) {
    let mut pool_account = runtime.view_account(&POOL_ACCOUNT_ID.into()).unwrap();
    pool_account.locked += amount;
    runtime.force_account_update(POOL_ACCOUNT_ID.into(), &pool_account);
}

fn wait_epoch(runtime: &mut RuntimeStandalone) {
    let epoch_height = runtime.current_block().epoch_height;
    while epoch_height == runtime.current_block().epoch_height {
        runtime.produce_block().unwrap();
    }
}
