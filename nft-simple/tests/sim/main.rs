use near_sdk::json_types::U128;
use near_sdk::serde_json::json;
use near_sdk::Balance;
use near_sdk_sim::transaction::ExecutionStatus;
use near_sdk_sim::{init_simulator, to_yocto, UserAccount, DEFAULT_GAS, STORAGE_AMOUNT};
use nft_simple::JsonToken;
use test_exquisite_corpse::ExquisiteCorpse;

// Load in contract bytes at runtime
near_sdk_sim::lazy_static_include::lazy_static_include_bytes! {
    NFT_WASM_BYTES => "res/nft_simple.wasm",
    FT_WASM_BYTES => "res/fungible_token.wasm",
    EC_WASM_BYTES => "res/test_exquisite_corpse.wasm",
}

type TokenId = String;

const NFT_ID: &str = "nft.sim";
const FT_ID: &str = "ndai";
const EXQUISITE_CORPSE_ID: &str = "ec.sim";
const APPROVAL_COST: Balance = 210_000_000_000_000_000_000;
const FT_STORAGE_COST: Balance = 1_250_000_000_000_000_000_000;

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

fn helper_nft_transfer(nft: &UserAccount, token_id: TokenId, recipient: &UserAccount) {
    nft.call(
        nft.account_id(),
        "nft_transfer",
        &json!({
            "receiver_id": recipient.account_id(),
            "token_id": token_id
        })
        .to_string()
        .into_bytes(),
        DEFAULT_GAS,
        1, // deposit
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
fn simulate_exquisite_corpse_interactions() {
    let (root_account, nft) = sim_helper_init();
    let (alice, _) = sim_helper_create_alice_bob(&root_account);

    // Mint a number of tokens to be sent or approved to EC
    helper_mint(&nft, "0".to_string()); // transfer head
    helper_mint(&nft, "1".to_string()); // transfer midsection
    helper_mint(&nft, "2".to_string()); // transfer midsection
    helper_mint(&nft, "3".to_string()); // transfer feet
    helper_mint(&nft, "4".to_string()); // approve head
    helper_mint(&nft, "5".to_string()); // approve head
    helper_mint(&nft, "6".to_string()); // approve midsection
    helper_mint(&nft, "7".to_string()); // approve feet

    let hundred_near = to_yocto("100");
    let artist_head = root_account.create_user("artist-head.sim".to_string(), hundred_near);
    let artist_mid = root_account.create_user("artist-midsection.sim".to_string(), hundred_near);
    let artist_feet = root_account.create_user("artist-feet.sim".to_string(), hundred_near);

    helper_nft_transfer(&nft, "0".to_string(), &artist_head);
    helper_nft_transfer(&nft, "1".to_string(), &artist_mid);
    helper_nft_transfer(&nft, "2".to_string(), &artist_mid);
    helper_nft_transfer(&nft, "3".to_string(), &artist_feet);
    helper_nft_transfer(&nft, "4".to_string(), &artist_head);
    helper_nft_transfer(&nft, "5".to_string(), &artist_head);
    helper_nft_transfer(&nft, "6".to_string(), &artist_mid);
    helper_nft_transfer(&nft, "7".to_string(), &artist_feet);

    // Set up exquisite corpse
    let ec = sim_helper_init_ec(&root_account);

    // Artists transfer and call NFTs to EC
    // Transfer index 0 (only head)
    artist_head
        .call(
            nft.account_id(),
            "nft_transfer_call",
            &json!({
                "receiver_id": ec.account_id(),
                "token_id": "0",
                "msg": r#"{"section": "head"}"#
            })
            .to_string()
            .into_bytes(),
            DEFAULT_GAS,
            1, // attached deposit
        )
        .assert_success();

    let token_info: JsonToken = nft
        .view(
            nft.account_id(),
            "nft_token",
            &json!({
                "receiver_id": ec.account_id(),
                "token_id": "0",
                "msg": r#"{"section": "head"}"#
            })
            .to_string()
            .into_bytes(),
        )
        .unwrap_json();

    // Ensure that new owner is the exquisite corpse contract.
    assert_eq!(
        token_info.owner_id,
        ec.account_id(),
        "Expected ec account to be the new owner."
    );

    // Transfer index 1 (first head)
    artist_mid
        .call(
            nft.account_id(),
            "nft_transfer_call",
            &json!({
                "receiver_id": ec.account_id(),
                "token_id": "1",
                "msg": r#"{"section": "mid"}"#
            })
            .to_string()
            .into_bytes(),
            DEFAULT_GAS,
            1, // attached deposit
        )
        .assert_success();
    // Transfer index 2 (second head)
    artist_mid
        .call(
            nft.account_id(),
            "nft_transfer_call",
            &json!({
                "receiver_id": ec.account_id(),
                "token_id": "2",
                "msg": r#"{"section": "mid"}"#
            })
            .to_string()
            .into_bytes(),
            DEFAULT_GAS,
            1, // attached deposit
        )
        .assert_success();

    // Ensure there are no auto-generated exquisite corpses yet.
    let mut exquisite_corpses: Vec<ExquisiteCorpse> = nft
        .view(ec.account_id(), "show_all_corpses", &[])
        .unwrap_json();

    assert_eq!(exquisite_corpses.len(), 0);

    // Once we add this, it'll automatically generate an exquisite corpse.
    // Transfer index 3 (only feet)
    artist_feet
        .call(
            nft.account_id(),
            "nft_transfer_call",
            &json!({
                "receiver_id": ec.account_id(),
                "token_id": "3",
                "msg": r#"{"section": "feet"}"#
            })
            .to_string()
            .into_bytes(),
            DEFAULT_GAS,
            1, // attached deposit
        )
        .assert_success();

    exquisite_corpses = nft
        .view(ec.account_id(), "show_all_corpses", &[])
        .unwrap_json();

    // This shows that an action has been taken using nft_transfer_call
    assert_eq!(exquisite_corpses.len(), 1);

    // Artists approve NFTs to EC
    // Transfer index 4 (first head)
    artist_head
        .call(
            nft.account_id(),
            "nft_approve",
            &json!({
                "token_id": "4",
                "account_id": ec.account_id(),
                "msg": r#"{"section": "head"}"#
            })
            .to_string()
            .into_bytes(),
            DEFAULT_GAS,
            APPROVAL_COST, // attached deposit
        )
        .assert_success();
    // Transfer index 5 (second head)
    artist_head
        .call(
            nft.account_id(),
            "nft_approve",
            &json!({
                "token_id": "5",
                "account_id": ec.account_id(),
                "msg": r#"{"section": "head"}"#
            })
            .to_string()
            .into_bytes(),
            DEFAULT_GAS,
            APPROVAL_COST, // attached deposit
        )
        .assert_success();

    // Transfer index 6 (only midsection)
    artist_mid
        .call(
            nft.account_id(),
            "nft_approve",
            &json!({
                "token_id": "6",
                "account_id": ec.account_id(),
                "msg": r#"{"section": "mid"}"#
            })
            .to_string()
            .into_bytes(),
            DEFAULT_GAS,
            APPROVAL_COST, // attached deposit
        )
        .assert_success();

    // Should not be able to manually create an exquisite corpse until there are feet.
    let mut can_manually_create_ec: bool = root_account
        .view(ec.account_id(), "can_create_corpse_from_approvals", &[])
        .unwrap_json();
    assert!(!can_manually_create_ec, "Shouldn't be able to manually create an exquisite corpse until all pieces have been approved.");

    // Transfer index 7 (only feet)
    artist_feet
        .call(
            nft.account_id(),
            "nft_approve",
            &json!({
                "token_id": "7",
                "account_id": ec.account_id(),
                "msg": r#"{"section": "feet"}"#
            })
            .to_string()
            .into_bytes(),
            DEFAULT_GAS,
            APPROVAL_COST, // attached deposit
        )
        .assert_success();

    // Now a user can create a manual exquisite corpse
    can_manually_create_ec = root_account
        .view(ec.account_id(), "can_create_corpse_from_approvals", &[])
        .unwrap_json();
    assert!(
        can_manually_create_ec,
        "Should be able to manually create an exquisite corpse once all pieces have been approved."
    );

    // Set up fungible token to purchase an exquisite corpse selection, since it costs 10 tokens from "ndai"
    let ft = sim_helper_init_ft(&root_account);

    let alice_balance: U128 = alice
        .view(
            ft.account_id(),
            "ft_balance_of",
            &json!({
                "account_id": alice.account_id()
            })
            .to_string()
            .into_bytes(),
        )
        .unwrap_json();
    assert_eq!(alice_balance.0, 0u128);

    // Set storage for Alice and the Exquisite Corpse contract
    alice
        .call(
            ft.account_id(),
            "storage_deposit",
            &json!({
                "account_id": alice.account_id()
            })
            .to_string()
            .into_bytes(),
            DEFAULT_GAS,
            FT_STORAGE_COST, // attached deposit
        )
        .assert_success();
    ec.call(
        ft.account_id(),
        "storage_deposit",
        &json!({
            "account_id": ec.account_id()
        })
        .to_string()
        .into_bytes(),
        DEFAULT_GAS,
        FT_STORAGE_COST, // attached deposit
    )
    .assert_success();

    // Transfer from tokens to Alice.
    ft.call(
        ft.account_id(),
        "ft_transfer",
        &json!({
            "receiver_id": alice.account_id(),
            "amount": U128::from(200)
        })
        .to_string()
        .into_bytes(),
        DEFAULT_GAS,
        1, // deposit
    )
    .assert_success();

    /*
       receiver_id: ValidAccountId,
       amount: U128,
       memo: Option<String>,
       msg: String,
    */
    let num_tokens_used: U128 = alice
        .call(
            ft.account_id(),
            "ft_transfer_call",
            &json!({
                "receiver_id": ec.account_id(),
                "amount": U128::from(10),
                "msg": r#"{
            "head": {
                "index": "1",
                "token_contract": "nft.sim",
                "token_id": "5",
                "previous_owner": "artist-head.sim"
            },
            "mid": {
                "index": "0",
                "token_contract": "nft.sim",
                "token_id": "6",
                "previous_owner": "artist-midsection.sim"
            },
            "feet": {
                "index": "0",
                "token_contract": "nft.sim",
                "token_id": "7",
                "previous_owner": "artist-feet.sim"
            }}"#
            })
            .to_string()
            .into_bytes(),
            DEFAULT_GAS,
            1, // deposit
        )
        .unwrap_json();

    exquisite_corpses = nft
        .view(ec.account_id(), "show_all_corpses", &[])
        .unwrap_json();

    // This shows that an action has been taken using nft_transfer_call
    assert_eq!(exquisite_corpses.len(), 2);

    // Ensure that all 10 tokens were used.
    assert_eq!(num_tokens_used.0, 10);
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

fn sim_helper_init_ft(root_account: &UserAccount) -> UserAccount {
    // Deploy fungible token and call "new" method
    let ft = root_account.deploy(&FT_WASM_BYTES, FT_ID.into(), STORAGE_AMOUNT);
    ft.call(
        ft.account_id(),
        "new_default_meta",
        &json!({
            "owner_id": ft.account_id(),
            "total_supply": "1000000"
        })
        .to_string()
        .into_bytes(),
        DEFAULT_GAS,
        0, // attached deposit
    )
    .assert_success();
    ft
}

fn sim_helper_init_ec(root_account: &UserAccount) -> UserAccount {
    // Deploy fungible token and call "new" method
    let ec = root_account.deploy(&EC_WASM_BYTES, EXQUISITE_CORPSE_ID.into(), STORAGE_AMOUNT);
    ec.call(
        ec.account_id(),
        "new",
        &[],
        DEFAULT_GAS,
        0, // attached deposit
    )
    .assert_success();
    ec
}
