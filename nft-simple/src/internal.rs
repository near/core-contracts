use crate::*;

/// Price per 1 byte of storage from mainnet config after `0.18` release and protocol version `42`.
/// It's 10 times lower than the genesis price.
const STORAGE_PRICE_PER_BYTE: Balance = 10_000_000_000_000_000_000;

pub(crate) fn prefix(account_id: &AccountId) -> Vec<u8> {
    format!("o{}", account_id).into_bytes()
}

pub(crate) fn assert_one_yocto() {
    assert_eq!(
        env::attached_deposit(),
        1,
        "Requires attached deposit of exactly 1 yoctoNEAR"
    )
}

pub(crate) fn assert_self() {
    assert_eq!(
        env::predecessor_account_id(),
        env::current_account_id(),
        "Method is private"
    );
}

impl Contract {
    pub(crate) fn assert_owner(&self) {
        assert_eq!(
            &env::predecessor_account_id(),
            &self.owner_id,
            "Owner's method"
        );
    }

    pub(crate) fn assert_enough_storage(&self) {
        assert!(
            env::account_balance() + env::account_locked_balance()
                >= Balance::from(
                    env::storage_usage()
                        + self.total_supply * self.extra_storage_in_bytes_per_token
                ) * STORAGE_PRICE_PER_BYTE
        );
    }

    pub(crate) fn internal_add_token_to_owner(
        &mut self,
        account_id: &AccountId,
        token_id: &TokenId,
    ) {
        let mut tokens_set = self
            .accounts
            .get(account_id)
            .unwrap_or_else(|| UnorderedSet::new(prefix(account_id)));
        tokens_set.insert(token_id);
        self.accounts.insert(account_id, &tokens_set);
    }

    pub(crate) fn internal_remove_token_from_owner(
        &mut self,
        account_id: &AccountId,
        token_id: &TokenId,
    ) {
        let mut tokens_set = self
            .accounts
            .get(account_id)
            .expect("Token should be owned by the sender");
        tokens_set.remove(token_id);
        if tokens_set.is_empty() {
            self.accounts.remove(account_id);
        } else {
            self.accounts.insert(account_id, &tokens_set);
        }
    }

    pub(crate) fn internal_transfer(
        &mut self,
        sender_id: &AccountId,
        receiver_id: &AccountId,
        token_id: &TokenId,
        memo: Option<String>,
    ) {
        assert_ne!(
            sender_id, receiver_id,
            "Sender and receiver should be different"
        );
        let mut token = self.tokens.get(token_id).expect("Token not found");
        assert_eq!(sender_id, &token.owner_id, "Sender doesn't own the token");
        self.internal_remove_token_from_owner(sender_id, token_id);
        self.internal_add_token_to_owner(receiver_id, token_id);
        token.owner_id = receiver_id.clone();
        self.tokens.insert(token_id, &token);
        env::log(
            format!(
                "Transfer {} from {} to {}",
                token_id, sender_id, receiver_id
            )
            .as_bytes(),
        );
        if let Some(memo) = memo {
            env::log(format!("Memo: {}", memo).as_bytes());
        }
    }
}
