use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedSet};
use near_sdk::json_types::ValidAccountId;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, near_bindgen, AccountId, Balance, Promise, StorageUsage};

use crate::internal::*;
pub use crate::mint::*;
pub use crate::nft_core::*;

mod internal;
mod mint;
mod nft_core;

#[global_allocator]
static ALLOC: near_sdk::wee_alloc::WeeAlloc<'_> = near_sdk::wee_alloc::WeeAlloc::INIT;

pub type TokenId = String;

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Token {
    pub owner_id: AccountId,
    pub meta: String,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct Contract {
    /// AccountID -> Account balance.
    pub accounts: LookupMap<AccountId, UnorderedSet<TokenId>>,

    pub tokens: LookupMap<TokenId, Token>,

    pub owner_id: AccountId,

    pub total_supply: u64,

    /// The storage size in bytes for one account.
    pub extra_storage_in_bytes_per_token: StorageUsage,
}

impl Default for Contract {
    fn default() -> Self {
        env::panic(b"Contract is not initialized");
    }
}

#[near_bindgen]
impl Contract {
    #[init]
    pub fn new(owner_id: ValidAccountId) -> Self {
        assert!(!env::state_exists(), "Already initialized");
        let mut this = Self {
            accounts: LookupMap::new(b"a".to_vec()),
            tokens: LookupMap::new(b"t".to_vec()),
            owner_id: owner_id.into(),
            total_supply: 0,
            extra_storage_in_bytes_per_token: 0,
        };

        let initial_storage_usage = env::storage_usage();
        let tmp_account_id = unsafe { String::from_utf8_unchecked(vec![b'a'; 64]) };
        let mut u = UnorderedSet::new(prefix(&tmp_account_id));
        u.insert(&tmp_account_id);
        this.extra_storage_in_bytes_per_token = env::storage_usage() - initial_storage_usage
            + (tmp_account_id.len() - this.owner_id.len()) as u64;
        this.accounts.remove(&tmp_account_id);
        this
    }
}
