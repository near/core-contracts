extern crate env_logger;
// #[war]
#[allow(unused_imports)]
#[macro_use]
extern crate log;
extern crate quickcheck;
#[macro_use(quickcheck)]
extern crate quickcheck_macros;
mod utils;

use near_primitives::types::{AccountId, Balance};
use near_sdk::json_types::U128;
use serde::de::DeserializeOwned;
use serde_json::json;
use utils::{init_pool, ntoy, POOL_ACCOUNT_ID};

use near_runtime_standalone::RuntimeStandalone;

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

#[quickcheck]
fn qc_should_stake(initial_balance: Balance) -> bool {
    let (mut runtime, root) = init_pool(23 * 10u128.pow(24));
    let bob = root.create_external(&mut runtime, "bob".into(), ntoy(100));

    // println!("{:?}", res);
    dbg!(root.pool_ping(&mut runtime));
    bob.pool_deposit(&mut runtime, initial_balance);
    bob.pool_stake(&mut runtime, initial_balance);
    let bob_stake: U128 = call_pool(
        &mut runtime,
        "get_account_staked_balance",
        json!({"account_id": "bob"}),
    );

    assert_eq!(bob_stake, initial_balance.into());

    // root.pool_deposit(&mut runtime, 10000);
    // root.pool_stake(&mut runtime, 10000);
    // wait_epoch(&mut runtime);
    // reward_pool(&mut runtime, 1000);
    // bob.pool_unstake(&mut runtime);
    // let alice = root.create_external(&mut runtime, "alice".into(), ntoy(100));

    // alice.
    // alice.pool_deposit(&mut runtime, 10000);

    // alice.pool_stake(&mut runtime, 1.into());
    return true;
}

// fn reward_pool(runtime: &mut RuntimeStandalone, amount: Balance) {
//     let mut pool_account = runtime.view_account(&POOL_ACCOUNT_ID.into()).unwrap();
//     pool_account.locked += amount;
//     runtime.force_account_update(POOL_ACCOUNT_ID.into(), &pool_account);
// }

// fn wait_epoch(runtime: &mut RuntimeStandalone) {
//     let epoch_height = runtime.current_block().epoch_height;
//     while epoch_height == runtime.current_block().epoch_height {
//         runtime.produce_block().unwrap();
//     }
// }

#[quickcheck]
fn qc_test_deposit_withdraw_standalone(inital_balance: Balance) -> bool {
    let deposit_amount = ntoy(inital_balance);
    let (mut runtime, root) = init_pool(ntoy(100));
    let bob = root.create_external(&mut runtime, "bob".into(), ntoy(100));
    bob.pool_deposit(&mut runtime, deposit_amount);
    let _res = bob.get_account_unstaked_balance(&runtime);

    assert_eq!(_res, deposit_amount);
    let _outcome = bob.pool_withdraw(&mut runtime, deposit_amount);
    // match outcome.status {
    //     ExecutionStatus::Failure(err) => panic!(err),
    //     ExecutionStatus::SuccessValue(val) => info!("{}", String::from_utf8(val).unwrap()),
    //     _ => ()
    // };
    bob.get_account_unstaked_balance(&runtime) == 0u128
}
