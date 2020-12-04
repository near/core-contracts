use crate::*;

#[near_bindgen]
impl FungibleToken {
    #[init]
    pub fn new() -> Self {
        assert!(!env::state_exists(), "Already initialized");
        Self {
            accounts: LookupMap::new(b"a".to_vec()),
            total_supply: 0u128
        }
    }

    /// Increments the `allowance` for `escrow_account_id` by `amount` on the account of the caller of this contract
    /// (`predecessor_id`) who is the balance owner.
    /// Requirements:
    /// * Caller of the method has to attach deposit enough to cover storage difference at the
    ///   fixed storage price defined in the contract.
    #[payable]
    pub fn inc_allowance(&mut self, escrow_account_id: AccountId, amount: U128) {
        let initial_storage = env::storage_usage();

        assert!(
            env::is_valid_account_id(escrow_account_id.as_bytes()),
            "New owner's account ID is invalid"
        );

        let escrow_account_id: AccountId = escrow_account_id.into();
        let owner_id = env::predecessor_account_id();
        if escrow_account_id == owner_id {
            env::panic(b"Can not increment allowance for yourself");
        }
        let mut account = self.get_account(&owner_id);
        let current_allowance = account.get_allowance(&escrow_account_id);
        account.set_allowance(&escrow_account_id, current_allowance.saturating_add(amount.0));
        self.set_account(&owner_id, &account);
        self.refund_storage(initial_storage);
    }

    /// Decrements the `allowance` for `escrow_account_id` by `amount` on the account of the caller of this contract
    /// (`predecessor_id`) who is the balance owner.
    /// Requirements:
    /// * Caller of the method has to attach deposit enough to cover storage difference at the
    ///   fixed storage price defined in the contract.
    #[payable]
    pub fn dec_allowance(&mut self, escrow_account_id: AccountId, amount: U128) {
        let initial_storage = env::storage_usage();
        let escrow_account_id: AccountId = escrow_account_id.into();
        let owner_id = env::predecessor_account_id();
        if escrow_account_id == owner_id {
            env::panic(b"Can not decrement allowance for yourself");
        }
        let mut account = self.get_account(&owner_id);
        let current_allowance = account.get_allowance(&escrow_account_id);
        account.set_allowance(&escrow_account_id, current_allowance.saturating_sub(amount.0));
        self.set_account(&owner_id, &account);
        self.refund_storage(initial_storage);
    }

    /// Transfers the `amount` of tokens from `owner_id` to the `new_owner_id`.
    /// Requirements:
    /// * Recipient of the wNear tokens cannot be this contract
    /// * `amount` should be a positive integer.
    /// * `owner_id` should have balance on the account greater or equal than the transfer `amount`.
    /// * If this function is called by an escrow account (`owner_id != predecessor_account_id`),
    ///   then the allowance of the caller of the function (`predecessor_account_id`) on
    ///   the account of `owner_id` should be greater or equal than the transfer `amount`.
    /// * Alternatively, if they have infinite approval, their approval amount wont be reduced.
    /// * Caller of the method has to attach deposit enough to cover storage difference at the
    ///   fixed storage price defined in the contract.
    #[payable]
    pub fn transfer_from(&mut self, owner_id: AccountId, new_owner_id: AccountId, amount: U128) {
        let initial_storage = env::storage_usage();
        let new_owner_id: AccountId = new_owner_id.into();

        // Stop people accidentally sending tokens to the contract
        assert_ne!(
            new_owner_id, env::current_account_id(),
            "Invalid transfer to this contract"
        );

        let amount = amount.into();
        if amount == 0 {
            env::panic(b"Can't transfer 0 tokens");
        }
        assert_ne!(
            owner_id, new_owner_id,
            "The new owner should be different from the current owner"
        );
        // Retrieving the account from the state.
        let mut account = self.get_account(&owner_id);

        // Checking and updating unlocked balance
        if account.balance < amount {
            env::panic(b"Not enough balance");
        }
        account.balance -= amount;

        // If transferring by escrow, need to check and update allowance.
        let escrow_account_id = env::predecessor_account_id();
        if escrow_account_id != owner_id {
            let allowance = account.get_allowance(&escrow_account_id);
            if allowance != std::u128::MAX {
                if allowance < amount {
                    env::panic(b"Not enough allowance");
                }
                account.set_allowance(&escrow_account_id, allowance - amount);
            }
        }

        // Saving the account back to the state.
        self.set_account(&owner_id, &account);

        // Deposit amount to the new owner and save the new account to the state.
        let mut new_account = self.get_account(&new_owner_id);
        new_account.balance += amount;
        self.set_account(&new_owner_id, &new_account);
        self.refund_storage(initial_storage);
    }

    /// Transfer `amount` of tokens from the caller of the contract (`predecessor_id`) to
    /// `new_owner_id`.
    /// Act the same was as `transfer_from` with `owner_id` equal to the caller of the contract
    /// (`predecessor_id`).
    /// Requirements:
    /// * Caller of the method has to attach deposit enough to cover storage difference at the
    ///   fixed storage price defined in the contract.
    #[payable]
    pub fn transfer(&mut self, new_owner_id: AccountId, amount: U128) {
        // NOTE: New owner's Account ID checked in transfer_from.
        // Storage fees are also refunded in transfer_from.
        self.transfer_from(env::predecessor_account_id(), new_owner_id, amount);
    }

    /// Returns total supply of tokens.
    pub fn get_total_supply(&self) -> U128 {
        self.total_supply.into()
    }

    /// Returns balance of the `owner_id` account.
    pub fn get_balance(&self, owner_id: AccountId) -> U128 {
        self.get_account(&owner_id).balance.into()
    }

    /// Returns current allowance of `escrow_account_id` for the account of `owner_id`.
    ///
    /// NOTE: Other contracts should not rely on this information, because by the moment a contract
    /// receives this information, the allowance may already be changed by the owner.
    /// So this method should only be used on the front-end to see the current allowance.
    pub fn get_allowance(&self, owner_id: AccountId, escrow_account_id: AccountId) -> U128 {
        let escrow_account_id: AccountId = escrow_account_id.into();
        self.get_account(&owner_id).get_allowance(&escrow_account_id).into()
    }
}
