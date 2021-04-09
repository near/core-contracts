use near_sdk::serde_json::json;
use near_sdk::Balance;
use near_sdk_sim::transaction::ExecutionStatus;
use near_sdk_sim::{init_simulator, to_yocto, UserAccount, DEFAULT_GAS, STORAGE_AMOUNT};
use nft_simple::JsonToken;

// Load in contract bytes at runtime
near_sdk_sim::lazy_static_include::lazy_static_include_bytes! {
    NFT_WASM_BYTES => "res/nft_simple.wasm",
    TEST_WASM_BYTES => "res/test_nft.wasm"
}

type TokenId = String;

const NFT_ID: &str = "nft.sim";
const TEST_ID: &str = "test.sim";
const APPROVAL_COST: Balance = 210_000_000_000_000_000_000;

fn helper_mint(nft: &UserAccount, token_id: TokenId) {
    nft.call(
        nft.account_id(),
        "nft_mint",
        &json!({
            "token_id": token_id,
            "metadata": {
                "title": "Black as the night"
            }
        })
        .to_string()
        .into_bytes(),
        DEFAULT_GAS,
        to_yocto("0.01"),
    )
    .assert_success();
}

#[test]
fn simulate_mint() {
    let (_, nft) = sim_helper_init();

    helper_mint(&nft, "0".to_string());
}

#[test]
fn simulate_simple_transfer() {
    let (root_account, nft) = sim_helper_init();
    let (alice, bob) = sim_helper_create_alice_bob(&root_account);

    helper_mint(&nft, "0".to_string());

    // Transfer from root to alice.
    nft.call(
        nft.account_id(),
        "nft_transfer",
        &json!({
            "receiver_id": alice.account_id(),
            "token_id": "0".to_string(),
        })
        .to_string()
        .into_bytes(),
        DEFAULT_GAS,
        1, // deposit
    )
    .assert_success();

    // Transfer from alice to bob
    alice
        .call(
            nft.account_id(),
            "nft_transfer",
            &json!({
                "receiver_id": bob.account_id(),
                "token_id": "0".to_string(),
            })
            .to_string()
            .into_bytes(),
            DEFAULT_GAS,
            1, // deposit
        )
        .assert_success();
}

#[test]
fn simulate_approval_workflow() {
    let (root_account, nft) = sim_helper_init();
    let (alice, bob) = sim_helper_create_alice_bob(&root_account);

    helper_mint(&nft, "0".to_string());

    // Add alice and bob as approvers
    nft.call(
        nft.account_id(),
        "nft_approve",
        &json!({
            "token_id": "0".to_string(),
            "account_id": alice.account_id(),
        })
        .to_string()
        .into_bytes(),
        DEFAULT_GAS,
        APPROVAL_COST, // deposit
    )
    .assert_success();
    nft.call(
        nft.account_id(),
        "nft_approve",
        &json!({
            "token_id": "0".to_string(),
            "account_id": bob.account_id(),
        })
        .to_string()
        .into_bytes(),
        DEFAULT_GAS,
        APPROVAL_COST, // deposit
    )
    .assert_success();

    let mut token_info: JsonToken = root_account
        .view(
            nft.account_id(),
            "nft_token",
            &json!({
                "token_id": "0"
            })
            .to_string()
            .into_bytes(),
        )
        .unwrap_json();

    assert_eq!(
        token_info.approved_account_ids.len(),
        2,
        "Expected two approvers."
    );

    // Transfer from root to alice.
    alice
        .call(
            nft.account_id(),
            "nft_transfer",
            &json!({
                "receiver_id": alice.account_id(),
                "token_id": "0".to_string(),
                "approval_id": "0".to_string(),
                "memo": "glad I got this before Bob"
            })
            .to_string()
            .into_bytes(),
            DEFAULT_GAS,
            1, // deposit
        )
        .assert_success();

    // Confirm that all approvals (ie to Bob) have been removed
    token_info = root_account
        .view(
            nft.account_id(),
            "nft_token",
            &json!({
                "token_id": "0"
            })
            .to_string()
            .into_bytes(),
        )
        .unwrap_json();
    assert_eq!(
        token_info.approved_account_ids.len(),
        0,
        "Expected no approvers after a transfer."
    );

    // Confirm that bob trying to move token fails
    let outcome = bob.call(
        nft.account_id(),
        "nft_transfer",
        &json!({
            "receiver_id": bob.account_id(),
            "token_id": "0".to_string(),
            "approval_id": "1".to_string(),
            "memo": "glad I got this before Bob"
        })
        .to_string()
        .into_bytes(),
        DEFAULT_GAS,
        1, // deposit
    );
    let status = outcome.status();
    if let ExecutionStatus::Failure(err) = status {
        // At this time, this is the way to check for error messages.
        assert_eq!(
            err.to_string(),
            "Action #0: Smart contract panicked: Unauthorized"
        );
    } else {
        panic!("Expected failure from Bob's attempted transfer.");
    }
}

#[test]
fn simulate_on_functions() {
    let (root_account, nft) = sim_helper_init();

    helper_mint(&nft, "0".to_string());
    helper_mint(&nft, "1".to_string());

    // Set up test contract
    let test_contract = root_account.deploy(&TEST_WASM_BYTES, TEST_ID.into(), STORAGE_AMOUNT);

    let approve_result = nft.call(
            nft.account_id(),
            "nft_approve",
            &json!({
                "token_id": "0",
                "account_id": test_contract.account_id(),
                "msg": ""
            })
            .to_string()
            .into_bytes(),
            DEFAULT_GAS,
            APPROVAL_COST, // attached deposit
        );
    let log = approve_result.logs();
    assert_eq!(log[0], "Approved correctly".to_string());

    // Test nft_transfer_call with message indicating it succeeds.
    let mut transfer_call_result = nft
        .call(
            nft.account_id(),
            "nft_transfer_call",
            &json!({
                "receiver_id": test_contract.account_id(),
                "token_id": "0",
                "msg": r#"{"should_succeed": true}"#
            })
            .to_string()
            .into_bytes(),
            DEFAULT_GAS,
            1, // attached deposit
        );
    let mut promise_results = transfer_call_result.promise_results();
    let mut ex_result = promise_results.get(2).unwrap().as_ref();
    assert_eq!(ex_result.unwrap().logs()[0], "Transferred correctly.".to_string());

    // Confirm the new owner
    let mut token_info: JsonToken = root_account
        .view(
            nft.account_id(),
            "nft_token",
            &json!({
                "token_id": "0"
            })
                .to_string()
                .into_bytes(),
        )
        .unwrap_json();

    assert_eq!(
        token_info.owner_id,
        test_contract.account_id(),
        "NFT didn't transfer ownership as expected."
    );

    // Test nft_transfer_call with message indicating it fails.
    transfer_call_result = nft
        .call(
            nft.account_id(),
            "nft_transfer_call",
            &json!({
                "receiver_id": test_contract.account_id(),
                "token_id": "1",
                "msg": r#"{"should_succeed": false}"#
            })
            .to_string()
            .into_bytes(),
        DEFAULT_GAS,
        1, // attached deposit
        );
    promise_results = transfer_call_result.promise_results();
    ex_result = promise_results.get(2).unwrap().as_ref();
    assert_eq!(ex_result.unwrap().logs()[0], "Did not transfer correctly, returning NFT.".to_string());

    // Ensure that token 1's ownership hasn't changed.
    token_info = root_account
        .view(
            nft.account_id(),
            "nft_token",
            &json!({
                "token_id": "1"
            })
            .to_string()
            .into_bytes(),
        )
        .unwrap_json();

    assert_eq!(
        token_info.owner_id,
        nft.account_id(),
        "NFT should not have transferred ownership."
    );
}

/// Basic initialization returning the "root account" for the simulator
/// and the NFT account with the contract deployed and initialized.
fn sim_helper_init() -> (UserAccount, UserAccount) {
    // Here the "Some" parameter indicates we needed modified genesis config. This is likely an issue with simulation tests at the time of this writing.
    let mut genesis = near_sdk_sim::runtime::GenesisConfig::default();
    genesis
        .runtime_config
        .transaction_costs
        .action_creation_config
        .function_call_cost
        .execution = 0u64;
    let mut root_account = init_simulator(Some(genesis));
    root_account = root_account.create_user("sim".to_string(), to_yocto("10000"));

    // Deploy non-function token and call "new" method
    let nft = root_account.deploy(&NFT_WASM_BYTES, NFT_ID.into(), STORAGE_AMOUNT);
    nft.call(
        nft.account_id(),
        "new",
        &json!({
            "owner_id": nft.account_id(),
            "metadata": {
                "spec": "nft-1.0.0",
                "name": "Digital chalkboard sketches",
                "symbol": "CHALK"
            },
        })
        .to_string()
        .into_bytes(),
        DEFAULT_GAS,
        0, // attached deposit
    )
    .assert_success();

    (root_account, nft)
}

fn sim_helper_create_alice_bob(root_account: &UserAccount) -> (UserAccount, UserAccount) {
    let hundred_near = to_yocto("100");
    let alice = root_account.create_user("alice.sim".to_string(), hundred_near);
    let bob = root_account.create_user("bob.sim".to_string(), hundred_near);
    (alice, bob)
}
