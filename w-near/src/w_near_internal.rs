use crate::*;

/******************************/
/* Internal methods for wNear */
/******************************/

impl FungibleToken {
    /// Internal method for minting an `amount` to `receiver_id` AccountId
    pub fn mint(&mut self, receiver_id: &AccountId, amount: Balance) {
        if self.total_supply == std::u128::MAX {
            env::panic(b"Total supply limit reached");
        }

        if std::u128::MAX - self.total_supply < amount {
            env::panic(b"Amount will exceed max permitted total supply");
        }

        let mut account = self.get_account(&receiver_id);
        account.balance += amount;
        self.set_account(&receiver_id, &account);

        // Increase total supply
        self.total_supply += amount;
    }

    /// Internal method for burning an `amount` from `owner_id` AccountId
    pub fn burn(&mut self, owner_id: &AccountId, amount: Balance) {
        let mut account = self.get_account(&owner_id);

        if account.balance < amount {
            env::panic(b"Burning more than the account balance");
        }

        account.balance -= amount;
        self.set_account(&owner_id, &account);

        // Decrease total supply
        self.total_supply -= amount;
    }

    /// Helper method to get the account details for `owner_id`.
    pub fn get_account(&self, owner_id: &AccountId) -> Account {
        assert!(env::is_valid_account_id(owner_id.as_bytes()), "Owner's account ID is invalid");
        let account_hash = env::sha256(owner_id.as_bytes());
        self.accounts.get(&account_hash).unwrap_or_else(|| Account::new(account_hash))
    }

    /// Helper method to set the account details for `owner_id` to the state.
    pub fn set_account(&mut self, owner_id: &AccountId, account: &Account) {
        let account_hash = env::sha256(owner_id.as_bytes());
        if account.balance > 0 || account.num_allowances > 0 {
            self.accounts.insert(&account_hash, &account);
        } else {
            self.accounts.remove(&account_hash);
        }
    }

    pub fn refund_storage(&self, initial_storage: StorageUsage) {
        let current_storage = env::storage_usage();
        let attached_deposit = env::attached_deposit();
        let refund_amount = if current_storage > initial_storage {
            let required_deposit =
                Balance::from(current_storage - initial_storage) * STORAGE_PRICE_PER_BYTE;
            assert!(
                required_deposit <= attached_deposit,
                "The required attached deposit is {}, but the given attached deposit is is {}",
                required_deposit,
                attached_deposit,
            );
            attached_deposit - required_deposit
        } else {
            attached_deposit
                + Balance::from(initial_storage - current_storage) * STORAGE_PRICE_PER_BYTE
        };
        if refund_amount > 0 {
            env::log(format!("Refunding {} tokens for storage", refund_amount).as_bytes());
            Promise::new(env::predecessor_account_id()).transfer(refund_amount);
        }
    }
}
