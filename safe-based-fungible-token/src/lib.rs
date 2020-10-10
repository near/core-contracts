use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::LookupMap;
use near_sdk::json_types::{ValidAccountId, U128};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, ext_contract, near_bindgen, AccountId, Balance, Gas, Promise};

#[global_allocator]
static ALLOC: near_sdk::wee_alloc::WeeAlloc<'_> = near_sdk::wee_alloc::WeeAlloc::INIT;

/// Price per 1 byte of storage from mainnet genesis config.
const STORAGE_PRICE_PER_BYTE: Balance = 100_000_000_000_000_000_000;

/// Don't need deposits for function calls.
const NO_DEPOSIT: Balance = 0;

/// NOTE: These fees are going to change with the update.
/// Basic compute.
const GAS_BASE_COMPUTE: Gas = 5_000_000_000_000;
/// Fee for function call promise.
const GAS_FOR_PROMISE: Gas = 5_000_000_000_000;
/// Fee for the `.then` call.
const GAS_FOR_DATA_DEPENDENCY: Gas = 10_000_000_000_000;

/// Gas attached to the receiver for `on_receive_with_safe` call.
/// NOTE: The minimum logic is to do some very basic compute and schedule a withdrawal from safe
/// that it returns from the promise.
const MIN_GAS_FOR_RECEIVER: Gas = GAS_FOR_PROMISE + GAS_BASE_COMPUTE;
/// Gas attached to the callback to resolve safe. It only needs to do basic compute.
/// NOTE: It doesn't account for storage refunds.
const GAS_FOR_CALLBACK: Gas = GAS_BASE_COMPUTE;
/// The amount of gas required to complete the execution of `transfer_with_safe`.
/// We need to create 2 promises with a dependencies and with some basic compute to write to the
/// state.
/// NOTE: It doesn't account for storage refunds.
const GAS_FOR_REMAINING_COMPUTE: Gas =
    2 * GAS_FOR_PROMISE + GAS_FOR_DATA_DEPENDENCY + GAS_BASE_COMPUTE;

/// Safe identifier.
#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone, Copy)]
#[serde(crate = "near_sdk::serde")]
pub struct SafeId(pub u64);

impl SafeId {
    pub fn next(&self) -> Self {
        Self(self.0 + 1)
    }
}

#[derive(BorshDeserialize, BorshSerialize, Clone, PartialEq)]
pub struct ShortAccountHash(pub [u8; 20]);

impl From<&AccountId> for ShortAccountHash {
    fn from(account_id: &AccountId) -> Self {
        let mut buf = [0u8; 20];
        buf.copy_from_slice(&env::sha256(account_id.as_bytes())[..20]);
        Self(buf)
    }
}

/// Contains balance and allowances information for one account.
#[derive(BorshDeserialize, BorshSerialize)]
pub struct Account {
    /// Current account balance.
    pub balance: Balance,
}

impl Default for Account {
    fn default() -> Self {
        Self { balance: 0 }
    }
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct Safe {
    /// The `ShortAccountHash` of the receiver ID.
    /// This information is only needed to validate safe ownership during withdrawal.
    pub receiver_id_hash: ShortAccountHash,
    /// The remaining amount of tokens in the safe.
    pub balance: Balance,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct SafeBasedFungibleToken {
    /// ShortAccountHash -> Account details.
    pub accounts: LookupMap<ShortAccountHash, Account>,

    /// Safes that currently exist.
    pub safes: LookupMap<SafeId, Safe>,

    /// The next safe ID to use.
    pub next_safe_id: SafeId,

    /// Total supply of the token. The sum of the all account balances.
    pub total_supply: Balance,
}

impl Default for SafeBasedFungibleToken {
    fn default() -> Self {
        env::panic(b"The contract is not initialized.");
    }
}

#[ext_contract(ext_token_receiver)]
pub trait ExtTokenReceiver {
    fn on_receive_with_safe(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        safe_id: SafeId,
        payload: String,
    );
}

#[ext_contract(ext_self)]
pub trait ExtSelf {
    fn resolve_safe(&mut self, safe_id: SafeId, sender_id: AccountId);
}

#[near_bindgen]
impl SafeBasedFungibleToken {
    /// Initializes the contract with the given total supply owned by the given `owner_id`.
    #[init]
    pub fn new(owner_id: ValidAccountId, total_supply: U128) -> Self {
        assert!(!env::state_exists(), "Already initialized");
        let total_supply = total_supply.into();
        let mut ft = Self {
            accounts: LookupMap::new(b"a".to_vec()),
            safes: LookupMap::new(b"s".to_vec()),
            next_safe_id: SafeId(0),
            total_supply,
        };
        ft.accounts.insert(
            &owner_id.as_ref().into(),
            &Account {
                balance: total_supply,
            },
        );
        ft
    }

    /// Simple transfers
    /// Gas requirement: 5 TGas or 5000000000000 Gas
    /// Should be called by the balance owner.
    /// Requires that the sender and the receiver accounts be registered.
    ///
    /// Actions:
    /// - Transfers `amount` of tokens from `predecessor_id` to `receiver_id`.
    pub fn transfer_unsafe(&mut self, receiver_id: ValidAccountId, amount: U128) {
        let amount = amount.into();
        self.withdraw_from_sender(receiver_id.as_ref(), amount);
        self.deposit_to_account(receiver_id.as_ref(), amount);
    }

    /// Transfer to a contract with payload
    /// Gas requirement: 40+ TGas or 40000000000000 Gas.
    /// Consumes: 30 TGas and the remaining gas is passed to the `receiver_id` (at least 10 TGas)
    /// Should be called by the balance owner.
    /// Returns a promise, that will result in the unspent balance from the transfer `amount`.
    ///
    /// Actions:
    /// - Withdraws `amount` from the `predecessor_id` account.
    /// - Creates a new local safe with a new unique `safe_id` with the following content:
    ///     `{sender_id: predecessor_id, amount: amount, receiver_id: receiver_id}`
    /// - Saves this safe to the storage.
    /// - Calls on `receiver_id` method `on_token_receive(sender_id: predecessor_id, amount, safe_id, payload)`/
    /// - Attaches a self callback to this promise `resolve_safe(safe_id, sender_id)`
    pub fn transfer_with_safe(
        &mut self,
        receiver_id: ValidAccountId,
        amount: U128,
        payload: String,
    ) -> Promise {
        let gas_to_receiver =
            env::prepaid_gas().saturating_sub(GAS_FOR_REMAINING_COMPUTE + GAS_FOR_CALLBACK);

        if gas_to_receiver < MIN_GAS_FOR_RECEIVER {
            env::panic(b"Not enough gas attached. Attach at least 40 TGas");
        }

        let amount = amount.into();
        let sender_id = self.withdraw_from_sender(receiver_id.as_ref(), amount);

        // Creating a new safe
        let safe_id = self.next_safe_id;
        self.next_safe_id = safe_id.next();
        let receiver_id_hash: ShortAccountHash = receiver_id.as_ref().into();
        let safe = Safe {
            receiver_id_hash,
            balance: amount,
        };
        self.safes.insert(&safe_id, &safe);

        // Calling the receiver
        ext_token_receiver::on_receive_with_safe(
            sender_id.clone(),
            amount.into(),
            safe_id,
            payload,
            receiver_id.as_ref(),
            NO_DEPOSIT,
            gas_to_receiver,
        )
        .then(ext_self::resolve_safe(
            safe_id,
            sender_id,
            &env::current_account_id(),
            NO_DEPOSIT,
            GAS_FOR_CALLBACK,
        ))
    }

    /// Withdraws from a given safe
    /// Gas requirement: 5 TGas or 5000000000000 Gas
    /// Should be called by the contract that owns a given safe.
    ///
    /// Actions:
    /// - checks that the safe with `safe_id` exists and `predecessor_id == safe.receiver_id`
    /// - withdraws `amount` from the safe or panics if `safe.amount < amount`
    /// - deposits `amount` on the `receiver_id`
    pub fn withdraw_from_safe(
        &mut self,
        safe_id: SafeId,
        receiver_id: ValidAccountId,
        amount: U128,
    ) {
        let mut safe = self.safes.get(&safe_id).expect("Safe doesn't exist");
        let safe_receiver_id = env::predecessor_account_id();
        if &ShortAccountHash::from(&safe_receiver_id) != &safe.receiver_id_hash {
            env::panic(b"The safe is not owned by the predecessor");
        }
        let amount = amount.into();
        if safe.balance < amount {
            env::panic(b"Not enough balance in the safe");
        }
        safe.balance -= amount;
        self.safes.insert(&safe_id, &safe);

        self.deposit_to_account(receiver_id.as_ref(), amount);
    }

    /// Resolves a given safe
    /// Gas requirement: 5 TGas or 5000000000000 Gas
    /// A callback. Should be called by this fungible token contract (`current_account_id`)
    /// Returns the remaining balance.
    ///
    /// Actions:
    /// - Reads safe with `safe_id`
    /// - Deposits remaining `safe.amount` to `sender_id`
    /// - Deletes the safe
    /// - Returns the remaining balance in the `safe.amount`.
    /// #[private]
    pub fn resolve_safe(&mut self, safe_id: SafeId, sender_id: AccountId) -> U128 {
        if env::current_account_id() != env::predecessor_account_id() {
            env::panic(b"This method is private");
        }

        let safe = self.safes.remove(&safe_id).expect("Safe doesn't exist");

        if safe.balance > 0 {
            let sender_id_hash = (&sender_id).into();
            if let Some(mut account) = self.accounts.get(&sender_id_hash) {
                account.balance += safe.balance;
                self.set_account(&sender_id_hash, &account);
            } else {
                self.total_supply -= safe.balance
            }
        }

        safe.balance.into()
    }

    /// Returns true if the account exists
    /// Gas requirement: 5 TGas or 5000000000000 Gas
    pub fn account_exists(&self, account_id: ValidAccountId) -> bool {
        self.accounts.contains_key(&account_id.as_ref().into())
    }

    /// Registers a given account.
    /// Gas requirement: 10 TGas or 10000000000000 Gas
    /// Requires deposit of 0.0077 NEAR
    ///
    /// Actions:
    /// - Verifies that the given account doesn't exist
    /// - Creates a new given account with 0 balance
    /// - Refunds the remaining deposit if more than the required deposit is attached.
    #[payable]
    pub fn register_account(&mut self, account_id: ValidAccountId) {
        let account_id_hash = account_id.as_ref().into();
        if self.accounts.contains_key(&account_id_hash) {
            env::panic(format!("Account {} already exists", account_id.as_ref()).as_bytes());
        }

        let storage_usage = env::storage_usage();
        self.accounts.insert(&account_id_hash, &Account::default());
        let storage_difference = env::storage_usage() - storage_usage;

        let attached_deposit = env::attached_deposit();
        let required_deposit = Balance::from(storage_difference) * STORAGE_PRICE_PER_BYTE;
        if attached_deposit < required_deposit {
            env::panic(
                format!(
                    "The attached deposit {} is less than the required deposit {}",
                    attached_deposit, required_deposit,
                )
                .as_bytes(),
            );
        }
        let refund_amount = attached_deposit - required_deposit;
        if refund_amount > 0 {
            env::log(format!("Refunding {} tokens for storage", refund_amount).as_bytes());
            Promise::new(env::predecessor_account_id()).transfer(refund_amount);
        }
    }

    /// Unregisters the account of the predecessor.
    /// Gas requirement: 10 TGas or 10000000000000 Gas
    /// Requires that the predecessor account exists and has no positive balance.
    ///
    /// Actions:
    /// - Verifies that the account exist
    /// - Creates a new given account with 0 balance
    /// - Refunds the amount release by storage.
    pub fn unregister_account(&mut self) {
        let account_id = env::predecessor_account_id();
        let storage_usage = env::storage_usage();
        if let Some(account) = self.accounts.remove(&(&account_id).into()) {
            if account.balance > 0 {
                env::panic(
                    format!(
                        "Can't unregister account {}, because it has a positive balance {}",
                        account_id, account.balance,
                    )
                    .as_bytes(),
                );
            }
        } else {
            env::panic(format!("Account {} doesn't exist", account_id).as_bytes())
        }
        let storage_difference = storage_usage - env::storage_usage();
        let refund_amount = Balance::from(storage_difference) * STORAGE_PRICE_PER_BYTE;
        env::log(format!("Refunding {} tokens for storage", refund_amount).as_bytes());
        Promise::new(account_id).transfer(refund_amount);
    }

    /// Returns total supply of tokens.
    pub fn get_total_supply(&self) -> U128 {
        self.total_supply.into()
    }

    /// Returns balance of the `owner_id` account.
    pub fn get_balance(&self, account_id: ValidAccountId) -> U128 {
        self.accounts
            .get(&account_id.as_ref().into())
            .map(|account| account.balance)
            .unwrap_or(0)
            .into()
    }
}

impl SafeBasedFungibleToken {
    /// Withdraws `amount` from the `predecessor_id` while comparing it to the `receiver_id`.
    /// Return `predecessor_id` and hash of the predecessor
    fn withdraw_from_sender(&mut self, receiver_id: &AccountId, amount: Balance) -> AccountId {
        if amount == 0 {
            env::panic(b"Transfer amount should be positive");
        }
        let sender_id = env::predecessor_account_id();
        if &sender_id == receiver_id {
            env::panic(b"The receiver should be different from the sender");
        }

        if receiver_id == &env::current_account_id() {
            env::panic(b"Can't transfer to this token contract");
        }

        // Retrieving the account from the state.
        let (mut account, sender_id_hash) = self.get_account_expect(&sender_id);

        // Checking and updating the balance
        if account.balance < amount {
            env::panic(b"Not enough balance");
        }
        account.balance -= amount;

        // Saving the account back to the state.
        self.set_account(&sender_id_hash, &account);

        sender_id
    }

    /// Deposits `amount` to the `account_id`
    fn deposit_to_account(&mut self, account_id: &AccountId, amount: Balance) {
        if amount == 0 {
            return;
        }
        // Retrieving the account from the state.
        let (mut account, account_id_hash) = self.get_account_expect(&account_id);
        account.balance += amount;
        // Saving the account back to the state.
        self.set_account(&account_id_hash, &account);
    }

    /// Helper method to get the account details for `owner_id`.
    fn get_account_expect(&self, account_id: &AccountId) -> (Account, ShortAccountHash) {
        let account_id_hash: ShortAccountHash = account_id.into();
        if let Some(account) = self.accounts.get(&account_id_hash) {
            (account, account_id_hash)
        } else {
            env::panic(format!("Account {} doesn't exist", account_id).as_bytes())
        }
    }

    /// Helper method to set the account details for `owner_id` to the state.
    fn set_account(&mut self, account_id_hash: &ShortAccountHash, account: &Account) {
        self.accounts.insert(account_id_hash, account);
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use near_sdk::{serde_json, MockedBlockchain};
    use near_sdk::{testing_env, VMContext};

    use super::*;
    use std::convert::TryInto;

    fn alice() -> ValidAccountId {
        "alice.near".try_into().unwrap()
    }
    fn bob() -> ValidAccountId {
        "bob.near".try_into().unwrap()
    }
    fn carol() -> ValidAccountId {
        "carol.near".try_into().unwrap()
    }

    fn get_context(
        predecessor_account_id: AccountId,
        is_view: bool,
        attached_deposit: Balance,
    ) -> VMContext {
        VMContext {
            current_account_id: alice().into(),
            signer_account_id: bob().into(),
            signer_account_pk: vec![0, 1, 2],
            predecessor_account_id,
            input: vec![],
            block_index: 0,
            block_timestamp: 0,
            account_balance: 1000 * 10u128.pow(24),
            account_locked_balance: 0,
            storage_usage: 10u64.pow(6),
            attached_deposit,
            prepaid_gas: 10u64.pow(18),
            random_seed: vec![0, 1, 2],
            is_view,
            output_data_receivers: vec![],
            epoch_height: 0,
        }
    }

    fn context(predecessor_account_id: ValidAccountId) {
        testing_env!(get_context(predecessor_account_id.into(), false, 0));
    }

    fn view_context() {
        testing_env!(get_context("view".to_string(), true, 0));
    }

    fn context_with_deposit(predecessor_account_id: ValidAccountId, attached_deposit: Balance) {
        testing_env!(get_context(
            predecessor_account_id.into(),
            false,
            attached_deposit
        ));
    }

    #[test]
    fn test_new() {
        context(carol());
        let total_supply = 1_000_000_000_000_000u128;
        let contract = SafeBasedFungibleToken::new(bob(), total_supply.into());
        view_context();
        assert_eq!(contract.get_total_supply().0, total_supply);
        assert_eq!(contract.get_balance(bob()).0, total_supply);
    }

    #[test]
    #[should_panic(expected = "The contract is not initialized")]
    fn test_default() {
        context(carol());
        let _contract = SafeBasedFungibleToken::default();
    }

    #[test]
    fn test_transfer_unsafe() {
        context(carol());
        let total_supply = 1_000_000_000_000_000u128;
        let mut contract = SafeBasedFungibleToken::new(carol(), total_supply.into());

        context_with_deposit(carol(), 77 * STORAGE_PRICE_PER_BYTE);
        contract.register_account(bob());
        // No refunds
        assert!(env::created_receipts().is_empty());

        context(carol());
        let transfer_amount = total_supply / 3;
        contract.transfer_unsafe(bob(), transfer_amount.into());

        view_context();
        assert_eq!(
            contract.get_balance(carol()).0,
            (total_supply - transfer_amount)
        );
        assert_eq!(contract.get_balance(bob()).0, transfer_amount);
    }

    #[test]
    #[should_panic(expected = "The receiver should be different from the sender")]
    fn test_transfer_unsafe_fail_self() {
        context(carol());
        let total_supply = 1_000_000_000_000_000u128;
        let mut contract = SafeBasedFungibleToken::new(carol(), total_supply.into());

        context(carol());
        let transfer_amount = total_supply / 3;
        contract.transfer_unsafe(carol(), transfer_amount.into());
    }

    #[test]
    fn test_transfer_with_safe() {
        context(carol());
        let total_supply = 1_000_000_000_000_000u128;
        let mut contract = SafeBasedFungibleToken::new(carol(), total_supply.into());

        context_with_deposit(bob(), 77 * STORAGE_PRICE_PER_BYTE);
        contract.register_account(bob());
        // No refunds
        assert!(env::created_receipts().is_empty());

        context(carol());
        let transfer_amount = total_supply / 3;
        contract.transfer_with_safe(bob(), transfer_amount.into(), "PAYLOAD".to_string());

        assert_eq!(contract.next_safe_id.0, 1);

        let receipts = env::created_receipts();

        assert_eq!(receipts.len(), 2);
        println!("{}", serde_json::to_string(&receipts[0]).unwrap());
        println!("{}", serde_json::to_string(&receipts[1]).unwrap());

        // Checking balances. Carol should have less, but bob still doesn't have it
        view_context();
        assert_eq!(
            contract.get_balance(carol()).0,
            (total_supply - transfer_amount)
        );
        assert_eq!(contract.get_balance(bob()).0, 0);

        // Assuming we're bob() and received
        // `on_receive_with_safe({"sender_id":"carol.near","amount":"333333333333333","safe_id":0,"payload":"PAYLOAD"})`.
        context(bob());
        let withdrawal_amount = transfer_amount / 2;
        contract.withdraw_from_safe(SafeId(0), bob(), withdrawal_amount.into());

        // Checking balances. Carol should have less, but Bob has withdrawal amount
        view_context();
        assert_eq!(
            contract.get_balance(carol()).0,
            (total_supply - transfer_amount)
        );
        assert_eq!(contract.get_balance(bob()).0, withdrawal_amount);

        // Resolving the safe
        context(alice());
        let res = contract.resolve_safe(SafeId(0), carol().into());
        assert_eq!(res.0, transfer_amount - withdrawal_amount);

        // Final balances
        view_context();
        assert_eq!(
            contract.get_balance(carol()).0,
            (total_supply - withdrawal_amount)
        );
        assert_eq!(contract.get_balance(bob()).0, withdrawal_amount);
    }
}
