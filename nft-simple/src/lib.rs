use std::collections::HashMap;

use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedMap, UnorderedSet};
use near_sdk::json_types::{ValidAccountId, Base64VecU8, U64};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, near_bindgen, AccountId, Balance, PanicOnDefault, Promise, StorageUsage};

use crate::internal::*;
pub use crate::mint::*;
pub use crate::nft_core::*;
use crate::nft_metadata::{TokenMetadata, NFTMetadata};

mod internal;
mod mint;
mod nft_core;
mod nft_metadata;

#[global_allocator]
static ALLOC: near_sdk::wee_alloc::WeeAlloc<'_> = near_sdk::wee_alloc::WeeAlloc::INIT;

pub type TokenId = String;

#[derive(Serialize, Deserialize, BorshDeserialize, BorshSerialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Token {
    pub owner_id: AccountId,
    pub metadata: TokenMetadata,
    pub approved_account_ids: HashMap<AccountId, U64>,
    pub approval_counter: U64,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {
    pub tokens_per_owner: LookupMap<AccountId, UnorderedSet<TokenId>>,

    pub tokens_by_id: UnorderedMap<TokenId, Token>,

    pub owner_id: AccountId,

    /// The storage size in bytes for one account.
    pub extra_storage_in_bytes_per_token: StorageUsage,

    pub metadata: NFTMetadata
}

#[near_bindgen]
impl Contract {
    #[init]
    pub fn new(owner_id: ValidAccountId, metadata: NFTMetadata) -> Self {
        assert!(!env::state_exists(), "Already initialized");
        let mut this = Self {
            tokens_per_owner: LookupMap::new(b"a".to_vec()),
            tokens_by_id: UnorderedMap::new(b"t".to_vec()),
            owner_id: owner_id.into(),
            extra_storage_in_bytes_per_token: 0,
            metadata
        };

        this.measure_min_token_storage_cost();

        this
    }

    fn measure_min_token_storage_cost(&mut self) {
        let initial_storage_usage = env::storage_usage();
        let tmp_account_id = "a".repeat(64);
        let u = UnorderedSet::new(unique_prefix(&tmp_account_id));
        self.tokens_per_owner.insert(&tmp_account_id, &u);

        let tokens_per_owner_entry_in_bytes = env::storage_usage() - initial_storage_usage;
        let owner_id_extra_cost_in_bytes = (tmp_account_id.len() - self.owner_id.len()) as u64;

        self.extra_storage_in_bytes_per_token =
            tokens_per_owner_entry_in_bytes + owner_id_extra_cost_in_bytes;

        self.tokens_per_owner.remove(&tmp_account_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::MockedBlockchain;
    use near_sdk::{testing_env, VMContext};
    use near_sdk::serde::export::TryFrom;

    fn alice() -> AccountId { String::from("alice.near") }
    fn bob() -> AccountId { String::from("bob.near") }
    fn nft() -> AccountId { String::from("nft.near") }

    fn get_context(predecessor_account_id: AccountId, attached_deposit: Balance) -> VMContext {
        VMContext {
            current_account_id: "alice_near".to_string(),
            signer_account_id: "bob_near".to_string(),
            signer_account_pk: vec![0, 1, 2],
            predecessor_account_id,
            input: vec![],
            block_index: 0,
            block_timestamp: 0,
            account_balance: 1000 * 10u128.pow(24),
            account_locked_balance: 0,
            storage_usage: 10u64.pow(6),
            attached_deposit,
            prepaid_gas: 2 * 10u64.pow(14),
            random_seed: vec![0, 1, 2],
            is_view: false,
            output_data_receivers: vec![],
            epoch_height: 19,
        }
    }

    fn helper_contract_metadata() -> NFTMetadata {
        NFTMetadata {
            spec: "".to_string(),
            name: "".to_string(),
            symbol: "".to_string(),
            icon: None,
            base_uri: None,
            reference: None,
            reference_hash: None
        }
    }

    fn helper_token_metadata() -> TokenMetadata {
        TokenMetadata {
            title: Some("Mochi Rising".to_string()),
            description: Some("Limited edition canvas".to_string()),
            media: None,
            media_hash: None,
            copies: None,
            issued_at: None,
            expires_at: None,
            starts_at: None,
            updated_at: None,
            extra: None,
            reference: None,
            reference_hash: None
        }
    }

    fn helper_mint() -> (Contract, VMContext) {
        let context = get_context(nft(), 7980000000000000000000);
        testing_env!(context.clone());
        let mut contract = Contract::new(ValidAccountId::try_from(nft()).unwrap(), helper_contract_metadata());
        contract.nft_mint("0".to_string(), helper_token_metadata());
        (contract, context)
    }

    #[test]
    fn basic_mint_from_owner() {
        helper_mint();
    }

    #[test]
    #[should_panic(expected = "Owner's method")]
    fn failed_mint_from_non_owner() {
        let context = get_context(alice(), 7660000000000000000000);
        testing_env!(context);
        let mut contract = Contract::new(ValidAccountId::try_from(nft()).unwrap(), helper_contract_metadata());
        contract.nft_mint("0".to_string(), helper_token_metadata());
    }

    #[test]
    fn simple_transfer() {
        let (mut contract, mut context) = helper_mint();
        let token_info = contract.nft_token("0".to_string());
        assert!(token_info.is_some(), "Expected to find newly minted token, got None.");
        let token_info_obj = token_info.unwrap();
        // Add one yoctoⓃ
        context.attached_deposit = 1;
        testing_env!(context.clone());
        contract.nft_transfer(ValidAccountId::try_from(bob()).unwrap(), "0".to_string(), None, Some("my memo".to_string()));
        assert_eq!(token_info_obj.approval_counter, U64::from(0), "Expected initial approval counter to be 0");
        assert_eq!(token_info_obj.approved_account_ids.len(), 0, "Expected number of initial approvers to be 0");
    }

    #[test]
    #[should_panic(expected = "Requires attached deposit of exactly 1 yoctoⓃ (0.000000000000000000000001 Ⓝ")]
    fn failed_simple_transfer_needs_one_yocto() {
        let (mut contract, _) = helper_mint();
        contract.nft_transfer(ValidAccountId::try_from(bob()).unwrap(), "0".to_string(), Some(U64::from(0u64)), Some("my memo".to_string()));
    }

    #[test]
    fn transfer_using_approver() {
        let (mut contract, mut context) = helper_mint();
        let mut token_info = contract.nft_token("0".to_string());
        assert!(token_info.is_some(), "Expected to find newly minted token, got None.");
        let mut token_info_obj = token_info.unwrap();
        assert_eq!(token_info_obj.approved_account_ids.len(), 0, "Expected no initial approvers.");
        contract.nft_approve("0".to_string(), ValidAccountId::try_from(alice()).unwrap(), None);
        token_info = contract.nft_token("0".to_string());
        assert!(token_info.is_some(), "Expected to find token after approval, got None.");
        token_info_obj = token_info.unwrap();
        assert_eq!(token_info_obj.approved_account_ids.len(), 1, "Expected one approver.");
        assert_eq!(token_info_obj.owner_id, nft(), "Expected nft.near to own token.");
        // Call from alice
        context.predecessor_account_id = alice();
        context.attached_deposit = 1;
        testing_env!(context.clone());
        contract.nft_transfer(ValidAccountId::try_from(alice()).unwrap(), "0".to_string(), Some(U64::from(1u64)), Some("thanks for allowing me to take it".to_string()));
        token_info = contract.nft_token("0".to_string());
        assert!(token_info.is_some(), "Expected to find token after transfer, got None.");
        token_info_obj = token_info.unwrap();
        assert_eq!(token_info_obj.approved_account_ids.len(), 0, "Expected approvers to reset to zero after transfer.");
        assert_eq!(token_info_obj.owner_id, alice(), "Expected alice.near to own token after transferring using approvals.");
    }

    #[test]
    #[should_panic(expected = "Unauthorized")]
    fn failed_transfer_using_unauthorized_approver() {
        let (mut contract, mut context) = helper_mint();
        contract.nft_approve("0".to_string(), ValidAccountId::try_from(alice()).unwrap(), None);
        // Bob tries to transfer when only alice should be allowed to
        context.predecessor_account_id = bob();
        context.attached_deposit = 1;
        testing_env!(context.clone());
        contract.nft_transfer(ValidAccountId::try_from(bob()).unwrap(), "0".to_string(), Some(U64::from(1u64)), Some("I am trying to hack you.".to_string()));
    }
}