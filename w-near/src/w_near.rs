use crate::*;
use near_sdk::json_types::U128;
use near_sdk::{assert_one_yocto, env, log, Promise};

#[near_bindgen]
impl Contract {
    /// Deposit NEAR to mint wNEAR tokens to the predecessor account in this contract.
    /// Requirements:
    /// * The predecessor account doesn't need to be registered.
    /// * Requires positive attached deposit.
    /// * If account is not registered will fail if attached deposit is below registration limit.
    #[payable]
    pub fn near_deposit(&mut self) {
        let mut amount = env::attached_deposit();
        assert!(amount > 0, "Requires positive attached deposit");
        let account_id = env::predecessor_account_id();
        if !self.ft.accounts.contains_key(&account_id) {
            // Not registered, register if enough $NEAR has been attached.
            // Subtract registration amount from the account balance.
            assert!(
                amount >= self.ft.storage_balance_bounds().min.0,
                "ERR_DEPOSIT_TOO_SMALL"
            );
            self.ft.internal_register_account(&account_id);
            amount -= self.ft.storage_balance_bounds().min.0;
        }
        self.ft.internal_deposit(&account_id, amount);
        log!("Deposit {} NEAR to {}", amount, account_id);
    }

    /// Withdraws wNEAR and send NEAR back to the predecessor account.
    /// Requirements:
    /// * The predecessor account should be registered.
    /// * `amount` must be a positive integer.
    /// * The predecessor account should have at least the `amount` of wNEAR tokens.
    /// * Requires attached deposit of exactly 1 yoctoNEAR.
    #[payable]
    pub fn near_withdraw(&mut self, amount: U128) -> Promise {
        assert_one_yocto();
        let account_id = env::predecessor_account_id();
        let amount = amount.into();
        self.ft.internal_withdraw(&account_id, amount);
        log!("Withdraw {} yoctoNEAR from {}", amount, account_id);
        // Transferring NEAR and refunding 1 yoctoNEAR.
        Promise::new(account_id).transfer(amount + 1)
    }
}
