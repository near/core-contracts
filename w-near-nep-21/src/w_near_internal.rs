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
}
