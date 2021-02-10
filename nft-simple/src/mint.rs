use crate::*;

#[near_bindgen]
impl Contract {
    #[payable]
    pub fn nft_mint(&mut self, token_id: TokenId, meta: String) {
        assert_one_yocto();
        self.assert_owner();
        let token = Token {
            owner_id: self.owner_id.clone(),
            meta,
        };
        assert!(
            self.tokens.insert(&token_id, &token).is_none(),
            "Token already exists"
        );
        self.internal_add_token_to_owner(&token.owner_id, &token_id);
        self.total_supply += 1;

        self.assert_enough_storage();
    }
}
