extern crate env_logger;
// #[war]
#[allow(unused_imports)]
#[macro_use]
extern crate log;
extern crate quickcheck;
#[macro_use(quickcheck)]
extern crate quickcheck_macros;
mod utils;

use near_primitives::{
    errors::TxExecutionError,
    transaction::ExecutionStatus,
    types::{AccountId, Balance},
};
use near_sdk::json_types::U128;
use serde::de::DeserializeOwned;
use serde_json::json;
use utils::{init_pool, ntoy, ExternalUser, POOL_ACCOUNT_ID};

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

#[allow(dead_code)]
fn check_invariants(_runtime: &mut RuntimeStandalone, _users: &[AccountId]) {}

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

fn create_with_user(
    initial_transfer: Balance,
    account_id: AccountId,
    initial_balance: Balance,
) -> (RuntimeStandalone, ExternalUser, ExternalUser) {
    let (mut runtime, root) = init_pool(initial_transfer);
    let bob = root.create_external(&mut runtime, account_id, initial_balance);
    (runtime, root, bob)
}

fn create_default(initial_balance: Balance) -> (RuntimeStandalone, ExternalUser, ExternalUser) {
    create_with_user(ntoy(100), "bob".into(), initial_balance)
}

#[quickcheck]
fn qc_test_deposit_withdraw_standalone(initial_balance: Balance) -> bool {
    let deposit_amount = ntoy(initial_balance + 1);
    let (mut runtime, _root, bob) = create_default(deposit_amount * 2);
    // let (mut runtime, root) = init_pool();
    // let bob = root.create_external(&mut runtime, "bob".into(), ntoy(100));
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

#[quickcheck]
fn qc_test_stake_unstake(initial_balance: Balance) -> bool {
    let deposit_amount = ntoy(initial_balance + 1);
    let (mut runtime, _root, bob) = create_default(ntoy(100) + deposit_amount);
    // let (mut runtime, root) = init_pool();
    // let bob = root.create_external(&mut runtime, "bob".into(), ntoy(100));
    bob.pool_deposit(&mut runtime, deposit_amount);
    let amount_to_stake = deposit_amount / 2;
    let _outcome = bob.pool_stake(&mut runtime, amount_to_stake);
    assert_eq!(bob.get_account_staked_balance(&runtime), amount_to_stake);
    bob.pool_unstake(&mut runtime, amount_to_stake);
    assert_eq!(bob.get_account_staked_balance(&runtime), 0);
    assert_eq!(bob.get_account_unstaked_balance(&runtime), deposit_amount);
    let mut res = bob.pool_withdraw(&mut runtime, amount_to_stake);
    match res.status {
        ExecutionStatus::Failure(TxExecutionError::ActionError(_)) => (),
        _ => panic!("shouldn't withdraw before epoch incremented"),
    };
    runtime.produce_blocks(3).unwrap();
    res = bob.pool_withdraw(&mut runtime, amount_to_stake);
    assert!(!matches!(res.status, ExecutionStatus::Failure(_)));
    return true;
}
