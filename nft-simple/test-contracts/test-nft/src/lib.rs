use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, near_bindgen, setup_alloc, AccountId, PromiseOrValue, serde_json};
use near_sdk::json_types::U64;

// Boilerplate for Rust smart contracts.
setup_alloc!();

type TokenId = String;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, Default)]
pub struct Contract {}

trait NonFungibleTokenReceiver {
    fn nft_on_transfer(
        &mut self,
        sender_id: AccountId,
        previous_owner_id: AccountId,
        token_id: TokenId,
        msg: String,
    ) -> PromiseOrValue<bool>;
}

trait NonFungibleTokenApprovalsReceiver {
    fn nft_on_approve(
        &mut self,
        token_id: TokenId,
        owner_id: AccountId,
        approval_id: U64,
        msg: String,
    );
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct NFTTransferMsg {
    should_succeed: bool,
}

#[near_bindgen]
impl NonFungibleTokenApprovalsReceiver for Contract {
    // This happens when a user adds an approval for their NFT on this account.
    #[allow(unused_variables)]
    fn nft_on_approve(&mut self, token_id: TokenId, owner_id: AccountId, approval_id: U64, msg: String) {
        env::log(b"Approved correctly");
    }
}

#[near_bindgen]
impl NonFungibleTokenReceiver for Contract {
    #[allow(unused_variables)]
    fn nft_on_transfer(&mut self, sender_id: AccountId, previous_owner_id: AccountId, token_id: TokenId, msg: String) -> PromiseOrValue<bool> {
        let msg_obj: NFTTransferMsg = serde_json::from_str(msg.as_str()).expect("msg parameter expects JSON in string form with one key: 'should_succeed' where the value is a boolean.");
        match msg_obj.should_succeed {
            true => {
                env::log(b"Transferred correctly.");
                PromiseOrValue::Value(false)
            },
            false => {
                env::log(b"Did not transfer correctly, returning NFT.");
                PromiseOrValue::Value(true)
            }
        }
    }
}
