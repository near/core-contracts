/*!
* wNear NEP-141 Token contract
*
* The aim of the contract is to enable the wrapping of the native NEAR token into a NEP-141 compatible token.
* It supports methods `near_deposit` and `near_withdraw` that wraps and unwraps NEAR tokens.
* They are effectively mint and burn underlying wNEAR tokens.
*
* lib.rs is the main entry point.
* w_near.rs contains interfaces for depositing and withdrawing
*/
use near_contract_standards::fungible_token::FungibleToken;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::json_types::{ValidAccountId, U128};
use near_sdk::Promise;
use near_sdk::{env, near_bindgen, PanicOnDefault};

mod w_near;

near_sdk::setup_alloc!();

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {
    pub ft: FungibleToken,
}

#[near_bindgen]
impl Contract {
    #[init]
    pub fn new() -> Self {
        assert!(!env::state_exists(), "Already initialized");
        Self {
            ft: FungibleToken::new(b"a"),
        }
    }
}

near_contract_standards::impl_fungible_token!(
    Contract,
    ft,
    String::from("0.1.0"),
    String::from("Wrapped NEAR fungible token"),
    String::from("wNEAR"),
    String::from("https://github.com/near/core-contracts/tree/master/w-near-141"),
    24
);
