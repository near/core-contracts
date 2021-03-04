use near_crypto::{InMemorySigner, KeyType, Signer};
use near_primitives::account::{AccessKey};
use near_sdk::serde_json::json;
use near_test::test_user::{init_test_runtime, to_yocto};

const DEFAULT_GAS: u64 = 300_000_000_000_000;

lazy_static::lazy_static! {
    static ref MULTISIG_WASM_BYTES: &'static [u8] = include_bytes!("../res/multisig.wasm").as_ref();
}

fn accounts(num: usize) -> String {
    ["root", "multisig"][num].to_string()
}

#[test]
fn setup_and_remove_multisig() {
    let mut runtime = init_test_runtime();
    let signer = InMemorySigner::from_seed(&accounts(0), KeyType::ED25519, "test");
    let _ = runtime.submit_transaction(
        runtime.transaction(accounts(0), accounts(1))
            .create_account()
            .transfer(to_yocto("100"))
            .deploy_contract(MULTISIG_WASM_BYTES.to_vec())
            .add_key(signer.public_key(), AccessKey::full_access())
            .function_call(
                "new".to_string(),
                json!({"num_confirmations": 1}).to_string().as_bytes().to_vec(),
                DEFAULT_GAS,
                0)).unwrap();
    let signer2 = InMemorySigner::from_seed(&accounts(1), KeyType::ED25519, "qqq");
    let args = json!({"request": {"receiver_id": accounts(1), "actions": [
        {"type": "AddKey", "public_key": signer2.public_key()},
        {"type": "DeployContract", "code": ""},
    ]}});
    let _ = runtime.call(accounts(1), accounts(1), "add_request", args, 0).unwrap();
    runtime.call(accounts(1), accounts(1), "confirm", json!({"request_id": 0}), 0).unwrap();
}
