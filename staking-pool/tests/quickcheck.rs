extern crate env_logger;
#[allow(unused_imports)]
#[macro_use]
extern crate log;
extern crate quickcheck;
#[macro_use(quickcheck)]
extern crate quickcheck_macros;
mod utils;

use near_primitives::types::{AccountId, Balance};
use near_sdk::json_types::U128;
use near_sdk::serde_json::{self, json};
use utils::{call_pool, init_pool, ntoy, wait_epoch};

use near_runtime_standalone::RuntimeStandalone;

#[allow(dead_code)]
fn check_invariants(_runtime: &mut RuntimeStandalone, _users: &[AccountId]) {}

#[quickcheck]
fn qc_should_stake(initial_balance: Balance) -> bool {
    let (mut runtime, root) = init_pool(ntoy(30));
    let bob = root
        .create_external(&mut runtime, "bob".into(), ntoy(100))
        .unwrap();

    let initial_balance = initial_balance + 1;

    bob.pool_deposit(&mut runtime, initial_balance).unwrap();
    bob.pool_stake(&mut runtime, initial_balance).unwrap();
    let bob_stake: U128 = call_pool(
        &mut runtime,
        "get_account_staked_balance",
        json!({"account_id": "bob"}),
    );

    assert_eq!(bob_stake, initial_balance.into());

    bob.pool_unstake(&mut runtime, initial_balance).unwrap();
    for _ in 0..4 {
        wait_epoch(&mut runtime);
    }
    runtime.produce_block().unwrap();
    let outcome = bob.pool_withdraw(&mut runtime, initial_balance);
    if let Err(outcome) = outcome {
        if initial_balance != 0 {
            panic!("{:?}", outcome);
        }
    };
    assert_eq!(bob.account(&mut runtime).amount, ntoy(100));
    return true;
}

#[quickcheck]
fn qc_test_deposit_withdraw_standalone(inital_balance: Balance) -> bool {
    let deposit_amount = ntoy(inital_balance);
    let (mut runtime, root) = init_pool(ntoy(100));
    let bob = root
        .create_external(&mut runtime, "bob".into(), ntoy(100))
        .unwrap();
    bob.pool_deposit(&mut runtime, deposit_amount).unwrap();
    let _res = bob.get_account_unstaked_balance(&runtime);

    assert_eq!(_res, deposit_amount);
    let outcome = bob.pool_withdraw(&mut runtime, deposit_amount);
    if let Err(outcome) = outcome {
        if deposit_amount != 0 {
            panic!("{:?}", outcome);
        }
    };
    bob.get_account_unstaked_balance(&runtime) == 0u128
}
