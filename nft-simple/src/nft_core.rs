use crate::*;
use near_sdk::json_types::{ValidAccountId, U64};
use near_sdk::{ext_contract, Gas, PromiseResult};

const GAS_FOR_RESOLVE_TRANSFER: Gas = 5_000_000_000_000;
const GAS_FOR_NFT_TRANSFER_CALL: Gas = 25_000_000_000_000 + GAS_FOR_RESOLVE_TRANSFER;

const NO_DEPOSIT: Balance = 0;

pub trait NonFungibleTokenCore {
    fn nft_transfer(
        &mut self,
        receiver_id: ValidAccountId,
        token_id: TokenId,
        memo: Option<String>,
    );

    /// Returns `true` if the token was transferred from the sender's account.
    fn nft_transfer_call(
        &mut self,
        receiver_id: ValidAccountId,
        token_id: TokenId,
        msg: String,
        memo: Option<String>,
    ) -> Promise;

    fn nft_total_supply(&self) -> U64;
}

#[ext_contract(ext_non_fungible_token_receiver)]
trait NonFungibleTokenReceiver {
    /// Returns `true` if the token should be returned back to the sender.
    /// TODO: Maybe make it inverse. E.g. true to keep it.
    fn nft_on_transfer(&mut self, sender_id: AccountId, token_id: TokenId, msg: String) -> Promise;
}

#[ext_contract(ext_self)]
trait NonFungibleTokenResolver {
    fn nft_resolve_transfer(
        &mut self,
        sender_id: AccountId,
        receiver_id: AccountId,
        token_id: TokenId,
    ) -> bool;
}

trait NonFungibleTokenResolver {
    fn nft_resolve_transfer(
        &mut self,
        sender_id: AccountId,
        receiver_id: AccountId,
        token_id: TokenId,
    ) -> bool;
}

#[near_bindgen]
impl NonFungibleTokenCore for Contract {
    #[payable]
    fn nft_transfer(
        &mut self,
        receiver_id: ValidAccountId,
        token_id: TokenId,
        memo: Option<String>,
    ) {
        assert_one_yocto();
        let sender_id = env::predecessor_account_id();
        self.internal_transfer(&sender_id, receiver_id.as_ref(), &token_id, memo);
    }

    #[payable]
    fn nft_transfer_call(
        &mut self,
        receiver_id: ValidAccountId,
        token_id: TokenId,
        msg: String,
        memo: Option<String>,
    ) -> Promise {
        assert_one_yocto();
        let sender_id = env::predecessor_account_id();
        self.internal_transfer(&sender_id, receiver_id.as_ref(), &token_id, memo);
        // Initiating receiver's call and the callback
        ext_non_fungible_token_receiver::nft_on_transfer(
            sender_id.clone(),
            token_id.clone(),
            msg,
            receiver_id.as_ref(),
            NO_DEPOSIT,
            env::prepaid_gas() - GAS_FOR_NFT_TRANSFER_CALL,
        )
        .then(ext_self::nft_resolve_transfer(
            sender_id,
            receiver_id.into(),
            token_id,
            &env::current_account_id(),
            NO_DEPOSIT,
            GAS_FOR_RESOLVE_TRANSFER,
        ))
    }

    fn nft_total_supply(&self) -> U64 {
        self.total_supply.into()
    }
}

#[near_bindgen]
impl NonFungibleTokenResolver for Contract {
    fn nft_resolve_transfer(
        &mut self,
        sender_id: AccountId,
        receiver_id: AccountId,
        token_id: TokenId,
    ) -> bool {
        assert_self();

        // Whether receiver wants to return token back to the sender, based on `nft_on_transfer`
        // call result.
        if let PromiseResult::Successful(value) = env::promise_result(0) {
            if let Ok(return_token) = near_sdk::serde_json::from_slice::<bool>(&value) {
                if !return_token {
                    // Token was successfully received.
                    return true;
                }
            }
        }

        let mut token = if let Some(token) = self.tokens.get(&token_id) {
            if &token.owner_id != &receiver_id {
                // The token is not owner by the receiver anymore. Can't return it.
                return true;
            }
            token
        } else {
            // The token was burned and doesn't exist anymore.
            return true;
        };

        self.internal_remove_token_from_owner(&receiver_id, &token_id);
        self.internal_add_token_to_owner(&sender_id, &token_id);
        token.owner_id = sender_id.clone();
        self.tokens.insert(&token_id, &token);

        env::log(format!("Return {} from {} to {}", token_id, receiver_id, sender_id).as_bytes());
        false
    }
}
