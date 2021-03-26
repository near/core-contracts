use crate::*;

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct NFTMetadata {
    spec: String, // required, essentially a version like "nft-1.0.0"
    name: String, // required, ex. "Mosaics"
    symbol: String, // required, ex. "MOSIAC"
    icon: Option<String>, // Data URL
    base_uri: Option<String>, // Centralized gateway known to have reliable access to decentralized storage assets referenced by `reference` or `media` URLs
    reference: Option<String>, // URL to a JSON file with more info
    reference_hash: Option<Base64VecU8>, // Base64-encoded sha256 hash of JSON from reference field. Required if `reference` is included.
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct TokenMetadata {
    title: Option<String>, // ex. "Arch Nemesis: Mail Carrier" or "Parcel #5055"
    description: Option<String>, // free-form description
    media: Option<String>, // URL to associated media, preferably to decentralized, content-addressed storage
    media_hash: Option<Base64VecU8>, // Base64-encoded sha256 hash of content referenced by the `media` field. Required if `media` is included.
    copies: Option<U64>, // number of copies of this set of metadata in existence when token was minted.
    issued_at: Option<String>, // ISO 8601 datetime when token was issued or minted
    expires_at: Option<String>, // ISO 8601 datetime when token expires
    starts_at: Option<String>, // ISO 8601 datetime when token starts being valid
    updated_at: Option<String>, // ISO 8601 datetime when token was last updated
    extra: Option<String>, // anything extra the NFT wants to store on-chain. Can be stringified JSON.
    reference: Option<String>, // URL to an off-chain JSON file with more info.
    reference_hash: Option<Base64VecU8>, // Base64-encoded sha256 hash of JSON from reference field. Required if `reference` is included.
}

pub trait NonFungibleTokenMetadata {
    fn nft_metadata(&self) -> NFTMetadata;
}

#[near_bindgen]
impl NonFungibleTokenMetadata for Contract {
    fn nft_metadata(&self) -> NFTMetadata {
        self.metadata.clone()
    }
}