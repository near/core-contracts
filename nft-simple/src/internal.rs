use crate::*;

/// Price per 1 byte of storage from mainnet config after `1.18.0` release and protocol version `42`.
/// It's 10 times lower than the genesis price.
pub(crate) const STORAGE_PRICE_PER_BYTE: Balance = 10_000_000_000_000_000_000;

pub(crate) fn unique_prefix(account_id: &AccountId) -> Vec<u8> {
    let mut prefix = Vec::with_capacity(33);
    prefix.push(b'o');
    prefix.extend(env::sha256(account_id.as_bytes()));
    prefix
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

pub(crate) fn deposit_refund(storage_used: u64) {
    let required_cost = STORAGE_PRICE_PER_BYTE * Balance::from(storage_used);
    let attached_deposit = env::attached_deposit();

    assert!(
        required_cost <= attached_deposit,
        "Requires to attach {} NEAR tokens to cover storage",
        required_cost
    );

    let refund = attached_deposit - required_cost;
    if refund > 0 {
        Promise::new(env::predecessor_account_id()).transfer(refund);
    }
}

pub(crate) fn bytes_for_approved_account_id(hm: (&AccountId, &u64)) -> u64 {
    // The extra 4 bytes are coming from Borsh serialization to store the length of the string.
    hm.0.len() as u64 + 4
}

pub(crate) fn refund_approved_account_ids(
    account_id: AccountId,
    approved_account_ids: &HashMap<AccountId, u64>,
) -> Promise {
    let storage_released: u64 = approved_account_ids
        .iter()
        .map(bytes_for_approved_account_id)
        .sum();
    Promise::new(account_id).transfer(Balance::from(storage_released) * STORAGE_PRICE_PER_BYTE)
}

impl Contract {
    pub(crate) fn assert_owner(&self) {
        assert_eq!(
            &env::predecessor_account_id(),
            &self.owner_id,
            "Owner's method"
        );
    }

    pub(crate) fn internal_add_token_to_owner(
        &mut self,
        account_id: &AccountId,
        token_id: &TokenId,
    ) {
        let mut tokens_set = self
            .tokens_per_owner
            .get(account_id)
            .unwrap_or_else(|| UnorderedSet::new(unique_prefix(account_id)));
        tokens_set.insert(token_id);
        self.tokens_per_owner.insert(account_id, &tokens_set);
    }

    pub(crate) fn internal_remove_token_from_owner(
        &mut self,
        account_id: &AccountId,
        token_id: &TokenId,
    ) {
        let mut tokens_set = self
            .tokens_per_owner
            .get(account_id)
            .expect("Token should be owned by the sender");
        tokens_set.remove(token_id);
        if tokens_set.is_empty() {
            self.tokens_per_owner.remove(account_id);
        } else {
            self.tokens_per_owner.insert(account_id, &tokens_set);
        }
    }

    pub(crate) fn internal_transfer(
        &mut self,
        sender_id: &AccountId,
        receiver_id: &AccountId,
        token_id: &TokenId,
        enforce_approval_id: Option<u64>,
        memo: Option<String>,
    ) -> (AccountId, HashMap<AccountId, u64>) {
        let Token {
            owner_id,
            metadata,
            approved_account_ids,
            approval_counter: approval_id,
        } = self.tokens_by_id.get(token_id).expect("Token not found");

        if sender_id != &owner_id && !approved_account_ids.contains_key(sender_id) {
            env::panic(b"Unauthorized");
        }

        if let Some(enforce_approval_id) = enforce_approval_id {
            assert_eq!(
                approval_id,
                enforce_approval_id,
                "The approval_id is different from enforce_approval_id"
            );
        }

        assert_ne!(
            &owner_id, receiver_id,
            "The token owner and the receiver should be different"
        );

        env::log(
            format!(
                "Transfer {} from @{} to @{}",
                token_id, &owner_id, receiver_id
            )
            .as_bytes(),
        );

        self.internal_remove_token_from_owner(&owner_id, token_id);
        self.internal_add_token_to_owner(receiver_id, token_id);

        let token = Token {
            owner_id: receiver_id.clone(),
            metadata,
            approved_account_ids: Default::default(),
            approval_counter: approval_id,
        };
        self.tokens_by_id.insert(token_id, &token);

        if let Some(memo) = memo {
            env::log(format!("Memo: {}", memo).as_bytes());
        }

        (owner_id, approved_account_ids)
    }
}

pub(crate) fn convert_token_to_ext_object(token: Token) -> TokenReturnObject {
    let mut json_map: HashMap<AccountId, U64> = HashMap::new();
    for hm in token.approved_account_ids.clone() {
        json_map.insert(hm.0, U64::from(hm.1));
    }

    TokenReturnObject {
        owner_id: token.owner_id,
        metadata: token.metadata,
        approved_account_ids: json_map,
        approval_counter: U64::from(token.approval_counter)
    }
}

pub(crate) fn convert_hashmap_to_ext_object(map: HashMap<AccountId, u64>) -> HashMap<AccountId, U64> {
    let mut json_map: HashMap<AccountId, U64> = HashMap::new();
    for entry in map {
        json_map.insert(entry.0, U64::from(entry.1));
    }
    json_map
}

pub(crate) fn convert_ext_hashmap_to_object(map: HashMap<AccountId, U64>) -> HashMap<AccountId, u64> {
    let mut reg_map: HashMap<AccountId, u64> = HashMap::new();
    for entry in map {
        reg_map.insert(entry.0, entry.1.into());
    }
    reg_map
}