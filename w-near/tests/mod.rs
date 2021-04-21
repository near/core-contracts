use near_sdk::json_types::U128;
use near_sdk::serde_json::json;
use near_sdk::Balance;
use near_sdk_sim::{
    call, deploy, init_simulator, to_yocto, view, ContractAccount, ExecutionResult, UserAccount,
    DEFAULT_GAS,
};
use w_near::ContractContract as Contract;

near_sdk_sim::lazy_static_include::lazy_static_include_bytes! {
    W_NEAR_WASM_BYTES => "res/w_near.wasm",
    LEGACY_W_NEAR_WASM_BYTES => "res/legacy_w_near.wasm",
}

const CONTRACT_ID: &str = "wrapnear";
const LEGACY_BYTE_COST: Balance = 10_000_000_000_000_000_000;

const STORAGE_BALANCE: Balance = 125 * LEGACY_BYTE_COST;

// Register the given `user` with Legacy wNEAR contract
fn legacy_register_user(user: &UserAccount) {
    user.call(
        CONTRACT_ID.to_string(),
        "storage_deposit",
        &json!({
            "account_id": user.valid_account_id()
        })
        .to_string()
        .into_bytes(),
        DEFAULT_GAS / 2,
        125 * LEGACY_BYTE_COST, // attached deposit
    )
    .assert_success();
}

fn wrap_near(user: &UserAccount, amount: Balance) -> ExecutionResult {
    user.call(
        CONTRACT_ID.to_string(),
        "near_deposit",
        &json!({
            "account_id": user.valid_account_id()
        })
        .to_string()
        .into_bytes(),
        DEFAULT_GAS / 2,
        amount, // attached deposit
    )
}

fn deploy_legacy() -> (UserAccount, ContractAccount<Contract>) {
    let root = init_simulator(None);
    let w_near = deploy!(
        contract: Contract,
        contract_id: CONTRACT_ID.to_string(),
        bytes: &LEGACY_W_NEAR_WASM_BYTES,
        signer_account: root,
        init_method: new()
    );
    (root, w_near)
}

fn deploy_w_near() -> (UserAccount, ContractAccount<Contract>) {
    let root = init_simulator(None);
    let w_near = deploy!(
        contract: Contract,
        contract_id: CONTRACT_ID.to_string(),
        bytes: &W_NEAR_WASM_BYTES,
        signer_account: root,
        init_method: new()
    );
    (root, w_near)
}

#[test]
pub fn test_upgrade() {
    let (root, w_near) = deploy_legacy();

    let legacy_storage_minimum_balance: U128 =
        view!(w_near.storage_minimum_balance()).unwrap_json();
    assert_eq!(legacy_storage_minimum_balance.0, STORAGE_BALANCE);

    let alice = root.create_user("alice".to_string(), to_yocto("100"));
    legacy_register_user(&alice);

    wrap_near(&alice, to_yocto("10")).assert_success();

    let alice_balance: U128 = view!(w_near.ft_balance_of(alice.valid_account_id())).unwrap_json();
    assert_eq!(alice_balance.0, to_yocto("10"));

    w_near
        .user_account
        .create_transaction(CONTRACT_ID.to_string())
        .deploy_contract(W_NEAR_WASM_BYTES.to_vec())
        .submit()
        .assert_success();

    let storage_minimum_balance: U128 = view!(w_near.storage_minimum_balance()).unwrap_json();
    assert_eq!(storage_minimum_balance.0, STORAGE_BALANCE);

    let alice_balance: U128 = view!(w_near.ft_balance_of(alice.valid_account_id())).unwrap_json();
    assert_eq!(alice_balance.0, to_yocto("10"));

    let bob = root.create_user("bob".to_string(), to_yocto("100"));
    legacy_register_user(&bob);

    wrap_near(&bob, to_yocto("15")).assert_success();

    let bob_balance: U128 = view!(w_near.ft_balance_of(bob.valid_account_id())).unwrap_json();
    assert_eq!(bob_balance.0, to_yocto("15"));

    call!(
        alice,
        w_near.ft_transfer(bob.valid_account_id(), to_yocto("5").into(), None),
        deposit = 1
    )
    .assert_success();

    let bob_balance: U128 = view!(w_near.ft_balance_of(bob.valid_account_id())).unwrap_json();
    assert_eq!(bob_balance.0, to_yocto("20"));
}

#[test]
pub fn test_legacy_ft_transfer() {
    let (root, w_near) = deploy_legacy();

    let alice = root.create_user("alice".to_string(), to_yocto("100"));
    legacy_register_user(&alice);

    wrap_near(&alice, to_yocto("10")).assert_success();

    let alice_balance: U128 = view!(w_near.ft_balance_of(alice.valid_account_id())).unwrap_json();
    assert_eq!(alice_balance.0, to_yocto("10"));

    let bob = root.create_user("bob".to_string(), to_yocto("100"));
    legacy_register_user(&bob);

    call!(
        alice,
        w_near.ft_transfer(bob.valid_account_id(), to_yocto("5").into(), None),
        deposit = 1
    )
    .assert_success();

    let bob_balance: U128 = view!(w_near.ft_balance_of(bob.valid_account_id())).unwrap_json();
    assert_eq!(bob_balance.0, to_yocto("5"));
}

#[test]
pub fn test_ft_transfer() {
    let (root, w_near) = deploy_w_near();

    let alice = root.create_user("alice".to_string(), to_yocto("100"));
    legacy_register_user(&alice);

    wrap_near(&alice, to_yocto("10")).assert_success();

    let alice_balance: U128 = view!(w_near.ft_balance_of(alice.valid_account_id())).unwrap_json();
    assert_eq!(alice_balance.0, to_yocto("10"));

    let bob = root.create_user("bob".to_string(), to_yocto("100"));
    legacy_register_user(&bob);

    call!(
        alice,
        w_near.ft_transfer(bob.valid_account_id(), to_yocto("5").into(), None),
        deposit = 1
    )
    .assert_success();

    let bob_balance: U128 = view!(w_near.ft_balance_of(bob.valid_account_id())).unwrap_json();
    assert_eq!(bob_balance.0, to_yocto("5"));
}

#[test]
pub fn test_legacy_wrap_fail() {
    let (root, _w_near) = deploy_legacy();

    let alice = root.create_user("alice".to_string(), to_yocto("100"));

    let status = wrap_near(&alice, to_yocto("10"));
    assert!(!status.is_ok())
}

#[test]
pub fn test_wrap_without_storage_deposit() {
    let (root, w_near) = deploy_w_near();

    let alice = root.create_user("alice".to_string(), to_yocto("100"));

    wrap_near(&alice, to_yocto("10")).assert_success();

    let alice_balance: U128 = view!(w_near.ft_balance_of(alice.valid_account_id())).unwrap_json();
    assert_eq!(alice_balance.0, to_yocto("10") - STORAGE_BALANCE);
}
