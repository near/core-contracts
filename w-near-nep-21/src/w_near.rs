use crate::*;

#[near_bindgen]
impl FungibleToken {
    /// Deposit NEAR and send wNear tokens to the predecessor account
    /// Requirements:
    /// * `amount` must be a positive integer
    /// * Caller of the method has to attach deposit enough to cover:
    ///   * The `amount` of wNear tokens being minted, and
    ///   * The storage difference at the fixed storage price defined in the contract.
    #[payable]
    pub fn deposit(&mut self, amount: U128) {
        // Proxy through to deposit_to() making the receiver_id the predecessor
        self.deposit_to(env::predecessor_account_id(), amount);
    }

    /// Deposit NEAR from the predecessor account and send wNear to a specific receiver_id
    /// Requirements:
    /// * `receiver_id` cannot be this contract
    /// * `receiver_id` must be a valid account Id
    /// * `amount` must be a positive integer
    /// * Caller of the method has to attach deposit enough to cover:
    ///   * The `amount` of wNear tokens being minted, and
    ///   * The storage difference at the fixed storage price defined in the contract.
    #[payable]
    pub fn deposit_to(&mut self, receiver_id: AccountId, amount: U128) {
        let initial_storage = env::storage_usage();

        // As attached deposit includes tokens for storage, deposit amount needs to be explicit
        let amount: Balance = amount.into();
        if amount == 0 {
            env::panic(b"Deposit amount must be greater than zero");
        }

        assert!(
            env::is_valid_account_id(receiver_id.as_bytes()),
            "New owner's account ID is invalid"
        );

        assert_ne!(
            receiver_id, env::current_account_id(),
            "Invalid transfer to this contract"
        );

        // Mint to receiver_id
        self.mint(&receiver_id, amount.clone());

        // Check we have enough attached deposit
        let current_storage = env::storage_usage();
        let attached_deposit = env::attached_deposit();
        let required_deposit_for_tokens_and_storage = if current_storage > initial_storage {
            (Balance::from(current_storage - initial_storage) * STORAGE_PRICE_PER_BYTE)
                + amount
        } else {
            amount
        };

        assert!(
            attached_deposit >= required_deposit_for_tokens_and_storage,
            "The required attached deposit is {}, but the given attached deposit is is {}",
            required_deposit_for_tokens_and_storage,
            attached_deposit,
        );

        env::log(format!("{} wNear tokens minted", amount).as_bytes());

        // Send back any money that is sent over value for required_deposit_for_tokens_and_storage
        let refund_amount = if attached_deposit > required_deposit_for_tokens_and_storage {
            attached_deposit - required_deposit_for_tokens_and_storage
        } else {
            0
        };

        if refund_amount > 0 {
            env::log(format!("Refunding {} excess tokens", refund_amount).as_bytes());
            Promise::new(env::predecessor_account_id()).transfer(refund_amount);
        }
    }

    /// Unwrap wNear and send Near back to the predecessor account
    /// Requirements:
    /// * `amount` must be a positive integer
    /// * Caller must have a balance that is greater than or equal to `amount`
    /// * Caller of the method has to attach deposit enough to cover storage difference at the
    ///   fixed storage price defined in the contract.
    #[payable]
    pub fn withdraw(&mut self, amount: U128) {
        // Proxy through to withdraw_to() sending the Near to the predecessor account
        self.withdraw_to(env::predecessor_account_id(), amount);
    }

    /// Unwraps wNear from the predecessor account and sends the Near to a specific receiver_id
    /// Requirements:
    /// * `receiver_id` cannot be this contract
    /// * `receiver_id` must be a valid account Id
    /// * `amount` should be a positive integer
    /// * Caller must have a balance that is greater than or equal to `amount`.
    /// * Caller of the method has to attach deposit enough to cover storage difference at the
    ///   fixed storage price defined in the contract.
    #[payable]
    pub fn withdraw_to(&mut self, receiver_id: AccountId, amount: U128) {
        let receiver_id: AccountId = receiver_id.into();
        let initial_storage = env::storage_usage();

        let amount: Balance = amount.into();
        if amount == 0 {
            env::panic(b"Withdrawal amount must be greater than zero");
        }

        assert!(
            env::is_valid_account_id(receiver_id.as_bytes()),
            "New owner's account ID is invalid"
        );

        assert_ne!(
            receiver_id, env::current_account_id(),
            "Invalid transfer to this contract"
        );

        // Decrease the predecessor's wNear balance and reduce total supply
        self.burn(&env::predecessor_account_id(), amount.clone());

        // Send near `amount` to receiver_id
        env::log(format!("Withdrawal of {} wNear", amount).as_bytes());
        Promise::new(receiver_id).transfer(amount);

        self.refund_storage(initial_storage);
    }

    /// The withdraw_from function allows to unwrap wNear from an owner wallet to a receiver_id wallet
    /// Requirements:
    /// * `receiver_id` of the Near tokens cannot be this contract
    /// * `receiver_id` must be a valid account Id
    /// * `receiver_id` cannot be the same as `owner_id`. Use `withdraw()` in that scenario.
    /// * `amount` should be a positive integer.
    /// * `owner_id` should have balance on the account greater or equal than the withdraw `amount`.
    /// * If this function is called by an escrow account (`owner_id != predecessor_account_id`),
    ///   then the allowance of the caller of the function (`predecessor_account_id`) on
    ///   the account of `owner_id` should be greater or equal than the transfer `amount`.
    /// * Alternatively, if they have infinite approval, their approval amount wont be reduced.
    /// * Caller of the method has to attach deposit enough to cover storage difference at the
    ///   fixed storage price defined in the contract.
    #[payable]
    pub fn withdraw_from(&mut self, owner_id: AccountId, receiver_id: AccountId, amount: U128) {
        let receiver_id: AccountId = receiver_id.into();
        let initial_storage = env::storage_usage();

        let amount: Balance = amount.into();
        if amount == 0 {
            env::panic(b"Withdrawal amount must be greater than zero");
        }

        assert_ne!(
            receiver_id, env::current_account_id(),
            "Invalid transfer to this contract"
        );

        assert_ne!(
            owner_id, receiver_id,
            "The new owner should be different from the current owner"
        );

        assert!(
            env::is_valid_account_id(receiver_id.as_bytes()),
            "New owner's account ID is invalid"
        );

        // If transferring by allowance, need to check and update allowance.
        let escrow_account_id = env::predecessor_account_id();
        if escrow_account_id != owner_id {
            let mut account = self.get_account(&owner_id);
            let allowance = account.get_allowance(&escrow_account_id);
            if allowance != std::u128::MAX {
                if allowance < amount {
                    env::panic(b"Not enough allowance");
                }
                account.set_allowance(&escrow_account_id, allowance - amount);
            }
        }


        self.burn(&owner_id, amount.clone());

        // Send near `amount` to receiver_id
        env::log(format!("Withdrawal of {} wNear", amount).as_bytes());
        Promise::new(receiver_id).transfer(amount);

        self.refund_storage(initial_storage);
    }
}
