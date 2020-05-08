mod utils;
use near_sdk::json_types::U128;
use serde_json::json;
use staking_pool::RewardFeeFraction;
use utils::{ntoy, ExternalUser, POOL_ACCOUNT_ID};

use near_runtime_standalone::init_runtime_and_signer;
#[test]
fn should_stake() {
    let (mut runtime, signer) = init_runtime_and_signer(&"root".into());
    let root = ExternalUser::new("root".into(), signer);
    let bob = root.create_external(&mut runtime, "bob".into(), ntoy(100));

    root.pool_init_new(
        &mut runtime,
        RewardFeeFraction {
            numerator: 10,
            denominator: 100,
        },
    );
    bob.pool_deposit(&mut runtime, ntoy(10));
    bob.pool_stake(&mut runtime, 1000.into());
    let bob_stake = runtime
        .view_method_call(
            &POOL_ACCOUNT_ID.into(),
            "get_account_staked_balance",
            json!({"account_id": "bob"}).to_string().as_bytes(),
        )
        .unwrap()
        .0;

    assert_eq!(
        serde_json::from_slice::<U128>(bob_stake.as_slice()).unwrap(),
        U128(1000)
    );
}
