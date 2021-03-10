use crate::*;
use near_sdk::json_types::{ValidAccountId, U64};
use near_sdk::{ext_contract, Gas, PromiseResult};

const GAS_FOR_RESOLVE_TRANSFER: Gas = 10_000_000_000_000;
const GAS_FOR_NFT_TRANSFER_CALL: Gas = 25_000_000_000_000 + GAS_FOR_RESOLVE_TRANSFER;

const NO_DEPOSIT: Balance = 0;

pub trait NonFungibleTokenCore {
    fn nft_transfer(
        &mut self,
        receiver_id: ValidAccountId,
        token_id: TokenId,
        enforce_owner_id: Option<ValidAccountId>,
        memo: Option<String>,
    );

    /// Returns `true` if the token was transferred from the sender's account.
    fn nft_transfer_call(
        &mut self,
        receiver_id: ValidAccountId,
        token_id: TokenId,
        enforce_owner_id: Option<ValidAccountId>,
        memo: Option<String>,
        msg: String,
    ) -> Promise;

    fn nft_approve_account_id(&mut self, token_id: TokenId, account_id: ValidAccountId) -> bool;

    fn nft_revoke_account_id(&mut self, token_id: TokenId, account_id: ValidAccountId) -> bool;

    fn nft_revoke_all(&mut self, token_id: TokenId);

    fn nft_total_supply(&self) -> U64;

    fn nft_token(&self, token_id: TokenId) -> Option<Token>;
}

#[ext_contract(ext_non_fungible_token_receiver)]
trait NonFungibleTokenReceiver {
    /// Returns `true` if the token should be returned back to the sender.
    /// TODO: Maybe make it inverse. E.g. true to keep it.
    fn nft_on_transfer(
        &mut self,
        sender_id: AccountId,
        previous_owner_id: AccountId,
        token_id: TokenId,
        msg: String,
    ) -> Promise;
}

#[ext_contract(ext_self)]
trait NonFungibleTokenResolver {
    fn nft_resolve_transfer(
        &mut self,
        owner_id: AccountId,
        receiver_id: AccountId,
        approved_account_ids: HashSet<AccountId>,
        token_id: TokenId,
    ) -> bool;
}

trait NonFungibleTokenResolver {
    fn nft_resolve_transfer(
        &mut self,
        owner_id: AccountId,
        receiver_id: AccountId,
        approved_account_ids: HashSet<AccountId>,
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
        enforce_owner_id: Option<ValidAccountId>,
        memo: Option<String>,
    ) {
        assert_one_yocto();
        let sender_id = env::predecessor_account_id();
        let (previous_owner_id, approved_account_ids) = self.internal_transfer(
            &sender_id,
            receiver_id.as_ref(),
            &token_id,
            enforce_owner_id.as_ref(),
            memo,
        );
        refund_approved_account_ids(previous_owner_id, &approved_account_ids);
    }

    #[payable]
    fn nft_transfer_call(
        &mut self,
        receiver_id: ValidAccountId,
        token_id: TokenId,
        enforce_owner_id: Option<ValidAccountId>,
        memo: Option<String>,
        msg: String,
    ) -> Promise {
        assert_one_yocto();
        let sender_id = env::predecessor_account_id();
        let (owner_id, approved_account_ids) = self.internal_transfer(
            &sender_id,
            receiver_id.as_ref(),
            &token_id,
            enforce_owner_id.as_ref(),
            memo,
        );
        // Initiating receiver's call and the callback
        ext_non_fungible_token_receiver::nft_on_transfer(
            sender_id.clone(),
            owner_id.clone(),
            token_id.clone(),
            msg,
            receiver_id.as_ref(),
            NO_DEPOSIT,
            env::prepaid_gas() - GAS_FOR_NFT_TRANSFER_CALL,
        )
        .then(ext_self::nft_resolve_transfer(
            owner_id,
            receiver_id.into(),
            approved_account_ids,
            token_id,
            &env::current_account_id(),
            NO_DEPOSIT,
            GAS_FOR_RESOLVE_TRANSFER,
        ))
    }

    #[payable]
    fn nft_approve_account_id(&mut self, token_id: TokenId, account_id: ValidAccountId) -> bool {
        let mut token = self.tokens_by_id.get(&token_id).expect("Token not found");
        assert_eq!(&env::predecessor_account_id(), &token.owner_id);
        let account_id: AccountId = account_id.into();
        let storage_used = bytes_for_approved_account_id(&account_id);
        if token.approved_account_ids.insert(account_id) {
            deposit_refund(storage_used);
            self.tokens_by_id.insert(&token_id, &token);
            true
        } else {
            false
        }
    }

    #[payable]
    fn nft_revoke_account_id(&mut self, token_id: TokenId, account_id: ValidAccountId) -> bool {
        assert_one_yocto();
        let mut token = self.tokens_by_id.get(&token_id).expect("Token not found");
        let predecessor_account_id = env::predecessor_account_id();
        assert_eq!(&predecessor_account_id, &token.owner_id);
        if token.approved_account_ids.remove(account_id.as_ref()) {
            let storage_released = bytes_for_approved_account_id(account_id.as_ref());
            Promise::new(env::predecessor_account_id())
                .transfer(Balance::from(storage_released) * STORAGE_PRICE_PER_BYTE);
            self.tokens_by_id.insert(&token_id, &token);
            true
        } else {
            false
        }
    }

    #[payable]
    fn nft_revoke_all(&mut self, token_id: TokenId) {
        assert_one_yocto();
        let mut token = self.tokens_by_id.get(&token_id).expect("Token not found");
        let predecessor_account_id = env::predecessor_account_id();
        assert_eq!(&predecessor_account_id, &token.owner_id);
        if !token.approved_account_ids.is_empty() {
            refund_approved_account_ids(predecessor_account_id, &token.approved_account_ids);
            token.approved_account_ids.clear();
            self.tokens_by_id.insert(&token_id, &token);
        }
    }

    fn nft_total_supply(&self) -> U64 {
        self.total_supply.into()
    }

    fn nft_token(&self, token_id: TokenId) -> Option<Token> {
        self.tokens_by_id.get(&token_id)
    }
}

#[near_bindgen]
impl NonFungibleTokenResolver for Contract {
    fn nft_resolve_transfer(
        &mut self,
        owner_id: AccountId,
        receiver_id: AccountId,
        approved_account_ids: HashSet<AccountId>,
        token_id: TokenId,
    ) -> bool {
        assert_self();

        // Whether receiver wants to return token back to the sender, based on `nft_on_transfer`
        // call result.
        if let PromiseResult::Successful(value) = env::promise_result(0) {
            if let Ok(return_token) = near_sdk::serde_json::from_slice::<bool>(&value) {
                if !return_token {
                    // Token was successfully received.
                    refund_approved_account_ids(owner_id, &approved_account_ids);
                    return true;
                }
            }
        }

        let mut token = if let Some(token) = self.tokens_by_id.get(&token_id) {
            if &token.owner_id != &receiver_id {
                // The token is not owner by the receiver anymore. Can't return it.
                refund_approved_account_ids(owner_id, &approved_account_ids);
                return true;
            }
            token
        } else {
            // The token was burned and doesn't exist anymore.
            refund_approved_account_ids(owner_id, &approved_account_ids);
            return true;
        };

        env::log(format!("Return {} from @{} to @{}", token_id, receiver_id, owner_id).as_bytes());

        self.internal_remove_token_from_owner(&receiver_id, &token_id);
        self.internal_add_token_to_owner(&owner_id, &token_id);
        token.owner_id = owner_id;
        refund_approved_account_ids(receiver_id, &token.approved_account_ids);
        token.approved_account_ids = approved_account_ids;
        self.tokens_by_id.insert(&token_id, &token);

        false
    }
}
