use std::cell::RefCell;
use std::rc::Rc;
use near_sdk::serde_json::json;
use near_sdk_sim::{to_yocto, DEFAULT_GAS, UserAccount};
use near_sdk_sim::account::AccessKey;
use near_sdk_sim::near_crypto::{InMemorySigner, KeyType, Signer};
use near_sdk_sim::runtime::{GenesisConfig, RuntimeStandalone};
use near_sdk_sim::state_record::StateRecord;
use near_primitives_core::account::Account as PrimitiveAccount;
use multisig2::MultisigMember;

near_sdk_sim::lazy_static_include::lazy_static_include_bytes! {
    MULTISIG_WASM_BYTES => "res/multisig2.wasm",
}

fn accounts(num: usize) -> String {
    ["root", "multisig"][num].to_string()
}

#[test]
fn setup_and_remove_multisig() {
    let mut genesis = GenesisConfig::default();

    // Set up signers for root and multisig
    let root_signer = genesis.init_root_signer(&accounts(0));
    let multisig_signer = InMemorySigner::from_seed(&accounts(1), KeyType::ED25519, &accounts(1));

    // Push multisig account to state_records
    genesis.state_records.push(StateRecord::Account {
        account_id: accounts(1),
        account: PrimitiveAccount {
            amount: to_yocto("100"),
            locked: 0,
            code_hash: Default::default(),
            storage_usage: 0,
        },
    });
    genesis.state_records.push(StateRecord::AccessKey {
        account_id: accounts(1),
        public_key: multisig_signer.clone().public_key(),
        access_key: AccessKey::full_access(),
    });

    let runtime = RuntimeStandalone::new_with_store(genesis);
    let runtime_rc = &Rc::new(RefCell::new(runtime));

    // Set up proper UserAccount objects for root and multisig
    let root_account = UserAccount::new(runtime_rc, accounts(0), root_signer.clone());
    let multisig_account = UserAccount::new(runtime_rc, accounts(1), multisig_signer.clone());

    // Set up arguments that will be passed into the "new" function
    let new_args = json!(
        {
            "members": [
                { "public_key": multisig_signer.public_key() }
            ],
            "num_confirmations": 1}
        ).to_string();

    // Deploy multisig contract to the account
    let deploy_tx = multisig_account.create_transaction(accounts(1));
    deploy_tx.deploy_contract(
        MULTISIG_WASM_BYTES.to_vec(),
    ).submit().assert_success();

    // Call the "new" initialization method
    multisig_account.call(
        accounts(1),
        "new",
        new_args.as_bytes(),
        DEFAULT_GAS,
        0
    ).assert_success();

    // Ensure that the new member is added
    let members: Vec<MultisigMember> = root_account.view(accounts(1), "get_members", &[]).unwrap_json();
    assert_eq!(members.len(), 1);

    // Set up arguments for "add_request" and call it
    let add_request_args = json!({"request": {"receiver_id": accounts(1), "actions": [
        {"type": "AddKey", "public_key": root_signer.public_key()},
        {"type": "DeployContract", "code": ""},
    ]}}).to_string();
    multisig_account.call(
        accounts(1),
        "add_request",
        add_request_args.as_bytes(),
        DEFAULT_GAS,
        0
    ).assert_success();
    // assert!(1 == 2, "aloha ");

    // Call "confirm"
    multisig_account.call(
        accounts(1),
        "confirm",
        json!({"request_id": 0}).to_string().as_bytes(),
        DEFAULT_GAS,
        0
    ).assert_success();
}
