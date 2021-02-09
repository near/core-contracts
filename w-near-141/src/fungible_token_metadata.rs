use super::*;
use near_sdk::serde::Serialize;

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct FungibleTokenMetadata {
    name: String,
    symbol: String,
    reference: String,
    decimals: u8,
}

pub trait FungibleTokenMetadataProvider {
    fn ft_metadata() -> FungibleTokenMetadata;
}

#[near_bindgen]
impl FungibleTokenMetadataProvider for Contract {
    fn ft_metadata() -> FungibleTokenMetadata {
        FungibleTokenMetadata {
            name: String::from("Wrapped NEAR fungible token"),
            symbol: String::from("wNEAR"),
            reference: String::from(
                "https://github.com/near/core-contracts/tree/master/w-near-141",
            ),
            decimals: 24,
        }
    }
}
