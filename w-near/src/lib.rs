/**
* wNear NEP21 Token contract
*
* NEP21 Based on:
* https://github.com/near/near-sdk-rs/blob/ab5c01ca4c61a6414484b69302b84e5ce3113f2f/examples/fungible-token/src/lib.rs
*
* The aim of the contract is to enable the wrapping of the native Ⓝ token into a NEP21 compatible token.
* It's the equivalent of wrapping ETH into wETH via the WETH. This contract is based on the functionality
* of the WETH9 and WETH10 Solidity smart contracts.
*/
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::LookupMap;
use near_sdk::json_types::U128;
use near_sdk::{env, near_bindgen, AccountId, Balance, Promise, StorageUsage};

#[global_allocator]
static ALLOC: near_sdk::wee_alloc::WeeAlloc<'_> = near_sdk::wee_alloc::WeeAlloc::INIT;

/// Price per 1 byte of storage from mainnet genesis config.
const STORAGE_PRICE_PER_BYTE: Balance = 100_000_000_000_000_000_000;

/// Contains balance and allowances information for one account.
#[derive(BorshDeserialize, BorshSerialize)]
pub struct Account {
    /// Current account balance.
    pub balance: Balance,
    /// Escrow Account ID hash to the allowance amount.
    /// Allowance is the amount of tokens the Escrow Account ID can spent on behalf of the account
    /// owner.
    pub allowances: LookupMap<Vec<u8>, Balance>,
    /// The number of allowances
    pub num_allowances: u32,
}

impl Account {
    /// Initializes a new Account with 0 balance and no allowances for a given `account_hash`.
    pub fn new(account_hash: Vec<u8>) -> Self {
        Self { balance: 0, allowances: LookupMap::new(account_hash), num_allowances: 0 }
    }

    /// Sets allowance for account `escrow_account_id` to `allowance`.
    pub fn set_allowance(&mut self, escrow_account_id: &AccountId, allowance: Balance) {
        let escrow_hash = env::sha256(escrow_account_id.as_bytes());
        if allowance > 0 {
            if self.allowances.insert(&escrow_hash, &allowance).is_none() {
                self.num_allowances += 1;
            }
        } else {
            if self.allowances.remove(&escrow_hash).is_some() {
                self.num_allowances -= 1;
            }
        }
    }

    /// Returns the allowance of account `escrow_account_id`.
    pub fn get_allowance(&self, escrow_account_id: &AccountId) -> Balance {
        let escrow_hash = env::sha256(escrow_account_id.as_bytes());
        self.allowances.get(&escrow_hash).unwrap_or(0)
    }
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct FungibleToken {
    /// sha256(AccountID) -> Account details.
    pub accounts: LookupMap<Vec<u8>, Account>,

    /// Total supply of the all token.
    pub total_supply: Balance,
}

impl Default for FungibleToken {
    fn default() -> Self {
        env::panic(b"Contract should be initialized before usage.")
    }
}

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

        assert_ne!(
            owner_id, receiver_id,
            "The new owner should be different from the current owner"
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
            "Escrow account ID is invalid"
        );
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
        assert!(
            env::is_valid_account_id(escrow_account_id.as_bytes()),
            "Escrow account ID is invalid"
        );
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

        // Stop people accidentally sending tokens to the contract
        assert_ne!(
            new_owner_id, env::current_account_id(),
            "Invalid transfer to this contract"
        );

        assert!(
            env::is_valid_account_id(new_owner_id.as_bytes()),
            "New owner's account ID is invalid"
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
        assert!(
            env::is_valid_account_id(escrow_account_id.as_bytes()),
            "Escrow account ID is invalid"
        );
        self.get_account(&owner_id).get_allowance(&escrow_account_id).into()
    }
}

impl FungibleToken {
    /// Internal method for minting an `amount` to `receiver_id` AccountId
    fn mint(&mut self, receiver_id: &AccountId, amount: Balance) {
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
    fn burn(&mut self, owner_id: &AccountId, amount: Balance) {
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
    fn get_account(&self, owner_id: &AccountId) -> Account {
        assert!(env::is_valid_account_id(owner_id.as_bytes()), "Owner's account ID is invalid");
        let account_hash = env::sha256(owner_id.as_bytes());
        self.accounts.get(&account_hash).unwrap_or_else(|| Account::new(account_hash))
    }

    /// Helper method to set the account details for `owner_id` to the state.
    fn set_account(&mut self, owner_id: &AccountId, account: &Account) {
        let account_hash = env::sha256(owner_id.as_bytes());
        if account.balance > 0 || account.num_allowances > 0 {
            self.accounts.insert(&account_hash, &account);
        } else {
            self.accounts.remove(&account_hash);
        }
    }

    fn refund_storage(&self, initial_storage: StorageUsage) {
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

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod w_near_tests {
    use near_sdk::MockedBlockchain;
    use near_sdk::{testing_env, VMContext};

    use super::*;

    const ZERO_U128: Balance = 0u128;

    fn alice() -> AccountId {
        "alice.near".to_string()
    }
    fn bob() -> AccountId {
        "bob.near".to_string()
    }
    fn carol() -> AccountId {
        "carol.near".to_string()
    }
    fn w_near() -> AccountId {
        "w_near.near".to_string()
    }
    fn invalid_account_id() -> AccountId {
        "".to_string()
    }

    fn get_context(predecessor_account_id: AccountId) -> VMContext {
        VMContext {
            current_account_id: w_near(),
            signer_account_id: bob(),
            signer_account_pk: vec![0, 1, 2],
            predecessor_account_id,
            input: vec![],
            block_index: 0,
            block_timestamp: 0,
            account_balance: 1000 * 10u128.pow(24),
            account_locked_balance: 0,
            storage_usage: 10u64.pow(6),
            attached_deposit: 0,
            prepaid_gas: 10u64.pow(18),
            random_seed: vec![0, 1, 2],
            is_view: false,
            output_data_receivers: vec![],
            epoch_height: 0,
        }
    }

    #[test]
    fn contract_creation_with_new() {
        testing_env!(get_context(carol()));
        //let total_supply = 1_000_000_000_000_000u128;
        let contract = FungibleToken::new();
        assert_eq!(contract.get_total_supply().0, ZERO_U128);
        assert_eq!(contract.get_balance(alice()).0, ZERO_U128);
        assert_eq!(contract.get_balance(bob()).0, ZERO_U128);
        assert_eq!(contract.get_balance(carol()).0, ZERO_U128);
    }

    #[test]
    #[should_panic(expected = "Contract should be initialized before usage.")]
    fn default_fails() {
        testing_env!(get_context(carol()));
        let _contract = FungibleToken::default();
    }

    #[test]
    fn deposit() {
        let mut context = get_context(carol());
        testing_env!(context.clone());

        let mut contract = FungibleToken::new();
        context.storage_usage = env::storage_usage();

        let deposit_amount = 1_000_000_000_000_000u128;
        context.attached_deposit = deposit_amount + (133 * STORAGE_PRICE_PER_BYTE);
        testing_env!(context.clone());

        //assert_eq!(contract.get_near_balance().0, 0);

        contract.deposit(deposit_amount.into());

        //assert_eq!(contract.get_near_balance().0, 0);

        // TODO: check contract balance == deposit amount
        assert_eq!(contract.get_balance(carol()).0, deposit_amount);
        assert_eq!(contract.get_total_supply().0, deposit_amount);
    }

    #[test]
    fn deposit_to_bob_from_carol() {
        let mut context = get_context(carol());
        testing_env!(context.clone());

        let mut contract = FungibleToken::new();
        context.storage_usage = env::storage_usage();

        let deposit_amount = 1_000_000_000_000_000u128;
        context.attached_deposit = deposit_amount + (133 * STORAGE_PRICE_PER_BYTE);
        testing_env!(context.clone());

        //assert_eq!(contract.get_near_balance().0, 0);

        contract.deposit_to(bob(), deposit_amount.into());

        //assert_eq!(contract.get_near_balance().0, 0);

        // TODO: check contract balance == deposit amount
        assert_eq!(contract.get_balance(carol()).0, 0);
        assert_eq!(contract.get_balance(bob()).0, deposit_amount);
        assert_eq!(contract.get_total_supply().0, deposit_amount);
    }

    #[test]
    #[should_panic(expected = "Deposit amount must be greater than zero")]
    fn deposit_fails_when_amount_is_zero() {
        let mut context = get_context(carol());
        testing_env!(context.clone());

        let mut contract = FungibleToken::new();
        context.storage_usage = env::storage_usage();
        context.attached_deposit = 0;
        testing_env!(context.clone());

        contract.deposit_to(bob(), ZERO_U128.into());
    }

    #[test]
    #[should_panic(expected = "New owner's account ID is invalid")]
    fn deposit_to_fails_when_recipient_is_invalid() {
        let mut context = get_context(carol());
        testing_env!(context.clone());

        let mut contract = FungibleToken::new();
        context.storage_usage = env::storage_usage();
        context.attached_deposit = 0;
        testing_env!(context.clone());

        contract.deposit_to(invalid_account_id(), (5u128).into());
    }

    #[test]
    #[should_panic(expected = "Invalid transfer to this contract")]
    fn deposit_to_fails_when_recipient_is_w_near_contract() {
        let mut context = get_context(carol());
        testing_env!(context.clone());

        let mut contract = FungibleToken::new();
        context.storage_usage = env::storage_usage();
        context.attached_deposit = 0;
        testing_env!(context.clone());

        contract.deposit_to(w_near(), (5u128).into());
    }

    #[test]
    #[should_panic(expected = "The required attached deposit is 13300001000000000000000, but the given attached deposit is is 13300000000000000000000")]
    fn deposit_to_fails_when_the_required_deposit_is_not_attached() {
        let mut context = get_context(carol());
        testing_env!(context.clone());

        let mut contract = FungibleToken::new();
        context.storage_usage = env::storage_usage();

        let deposit_amount = 1_000_000_000_000_000u128;
        context.attached_deposit = 133 * STORAGE_PRICE_PER_BYTE; // attach required storage but not deposit
        testing_env!(context.clone());

        //assert_eq!(contract.get_near_balance().0, 0);

        contract.deposit(deposit_amount.into());
    }

    #[test]
    fn simple_deposit_and_withdrawal() {
        let mut context = get_context(carol());
        testing_env!(context.clone());

        let mut contract = FungibleToken::new();
        context.storage_usage = env::storage_usage();

        let deposit_amount = 1_000_000_000_000_000u128;
        context.attached_deposit = deposit_amount.clone() + (133 * STORAGE_PRICE_PER_BYTE);
        testing_env!(context.clone());

        //assert_eq!(contract.get_near_balance().0, 0);

        contract.deposit(deposit_amount.clone().into());

        //assert_eq!(contract.get_near_balance().0, 0);

        // TODO: check contract balance == deposit amount
        assert_eq!(contract.get_balance(carol()).0, deposit_amount);
        assert_eq!(contract.get_total_supply().0, deposit_amount);

        contract.withdraw(deposit_amount.clone().into());

        assert_eq!(contract.get_balance(carol()).0, 0);
        assert_eq!(contract.get_total_supply().0, 0);
    }

    #[test]
    fn simple_deposit_by_carol_and_withdrawal_to_alice() {
        let mut context = get_context(carol());
        testing_env!(context.clone());

        let mut contract = FungibleToken::new();
        context.storage_usage = env::storage_usage();

        let deposit_amount = 1_000_000_000_000_000u128;
        context.attached_deposit = deposit_amount.clone() + (133 * STORAGE_PRICE_PER_BYTE);
        testing_env!(context.clone());

        //assert_eq!(contract.get_near_balance().0, 0);

        contract.deposit(deposit_amount.clone().into());

        //assert_eq!(contract.get_near_balance().0, 0);

        // TODO: check contract balance == deposit amount
        assert_eq!(contract.get_balance(carol()).0, deposit_amount);
        assert_eq!(contract.get_total_supply().0, deposit_amount);

        contract.withdraw_to(alice(), deposit_amount.clone().into());

        // TODO: check alice near balance has increased
        assert_eq!(contract.get_balance(carol()).0, 0);
        assert_eq!(contract.get_total_supply().0, 0);
    }

    #[test]
    #[should_panic(expected = "Withdrawal amount must be greater than zero")]
    fn withdraw_fails_when_withdrawal_amount_is_zero() {
        let mut context = get_context(carol());
        testing_env!(context.clone());

        let mut contract = FungibleToken::new();
        context.storage_usage = env::storage_usage();
        context.attached_deposit = 0;
        testing_env!(context.clone());

        contract.withdraw(ZERO_U128.into());
    }

    #[test]
    #[should_panic(expected = "New owner's account ID is invalid")]
    fn withdraw_to_fails_when_recipient_is_invalid() {
        let mut context = get_context(carol());
        testing_env!(context.clone());

        let mut contract = FungibleToken::new();
        context.storage_usage = env::storage_usage();
        context.attached_deposit = 0;
        testing_env!(context.clone());

        contract.withdraw_to(invalid_account_id(), (5u128).into());
    }

    #[test]
    #[should_panic(expected = "Invalid transfer to this contract")]
    fn withdraw_to_fails_when_recipient_is_w_near_contract() {
        let mut context = get_context(carol());
        testing_env!(context.clone());

        let mut contract = FungibleToken::new();
        context.storage_usage = env::storage_usage();
        context.attached_deposit = 0;
        testing_env!(context.clone());

        contract.withdraw_to(w_near(), (5u128).into());
    }

    #[test]
    #[should_panic(expected = "Burning more than the account balance")]
    fn withdraw_to_fails_when_carol_tries_to_withdraw_more_than_her_w_near_balance() {
        let mut context = get_context(carol());
        testing_env!(context.clone());

        let mut contract = FungibleToken::new();
        context.storage_usage = env::storage_usage();

        let deposit_amount = 1_000_000_000_000_000u128;
        context.attached_deposit = deposit_amount.clone() + (133 * STORAGE_PRICE_PER_BYTE);
        testing_env!(context.clone());

        //assert_eq!(contract.get_near_balance().0, 0);

        contract.deposit(deposit_amount.clone().into());

        //assert_eq!(contract.get_near_balance().0, 0);

        // TODO: check contract balance == deposit amount
        assert_eq!(contract.get_balance(carol()).0, deposit_amount);
        assert_eq!(contract.get_total_supply().0, deposit_amount);

        contract.withdraw((deposit_amount.clone()+1).into());
    }

    #[test]
    #[should_panic(expected = "Withdrawal amount must be greater than zero")]
    fn withdraw_from_fails_when_the_withdrawal_amount_is_zero() {
        let mut context = get_context(carol());
        testing_env!(context.clone());

        let mut contract = FungibleToken::new();
        context.storage_usage = env::storage_usage();
        context.attached_deposit = 0;
        testing_env!(context.clone());

        contract.withdraw_from(alice(), carol(), ZERO_U128.into());
    }

    #[test]
    #[should_panic(expected = "New owner's account ID is invalid")]
    fn withdraw_from_fails_when_the_recipient_is_invalid() {
        let mut context = get_context(carol());
        testing_env!(context.clone());

        let mut contract = FungibleToken::new();
        context.storage_usage = env::storage_usage();
        context.attached_deposit = 0;
        testing_env!(context.clone());

        contract.withdraw_from(alice(), invalid_account_id(), (5u128).into());
    }

    #[test]
    #[should_panic(expected = "Invalid transfer to this contract")]
    fn withdraw_from_fails_when_the_recipient_is_the_w_near_contract() {
        let mut context = get_context(carol());
        testing_env!(context.clone());

        let mut contract = FungibleToken::new();
        context.storage_usage = env::storage_usage();
        context.attached_deposit = 0;
        testing_env!(context.clone());

        contract.withdraw_from(alice(), w_near(), (5u128).into());
    }

    #[test]
    #[should_panic(expected = "The new owner should be different from the current owner")]
    fn withdraw_from_fails_when_the_owner_and_recipient_are_the_same() {
        let mut context = get_context(carol());
        testing_env!(context.clone());

        let mut contract = FungibleToken::new();
        context.storage_usage = env::storage_usage();
        context.attached_deposit = 0;
        testing_env!(context.clone());

        contract.withdraw_from(alice(), alice(), (5u128).into());
    }

    #[test]
    #[should_panic(expected = "Not enough allowance")]
    fn withdraw_from_fails_when_the_escrow_account_does_not_have_enough_allowance() {
        let mut context = get_context(carol());
        testing_env!(context.clone());

        let mut contract = FungibleToken::new();
        context.storage_usage = env::storage_usage();
        context.attached_deposit = 0;
        testing_env!(context.clone());

        contract.withdraw_from(alice(), bob(), (5u128).into());
    }

    #[test]
    #[should_panic(expected = "Burning more than the account balance")]
    fn withdraw_from_fails_when_trying_to_withdraw_more_than_owners_balance() {
        let mut context = get_context(carol());
        testing_env!(context.clone());

        let mut contract = FungibleToken::new();
        context.storage_usage = env::storage_usage();

        let deposit_amount = 1_000_000_000_000_000u128;
        context.attached_deposit = deposit_amount.clone() + (1000 * STORAGE_PRICE_PER_BYTE);
        testing_env!(context.clone());

        //assert_eq!(contract.get_near_balance().0, 0);

        // get some wNear for carol
        contract.deposit(deposit_amount.clone().into());

        //assert_eq!(contract.get_near_balance().0, 0);

        // TODO: check contract balance == deposit amount
        assert_eq!(contract.get_balance(carol()).0, deposit_amount);
        assert_eq!(contract.get_total_supply().0, deposit_amount);

        // give bob allowance to withdraw some tokens
        assert_eq!(contract.get_allowance(carol(), bob()), ZERO_U128.into());

        contract.inc_allowance(bob(), std::u128::MAX.into());

        assert_eq!(contract.get_allowance(carol(), bob()), std::u128::MAX.into());

        // switch to a context with bob
        let mut context = get_context(bob());
        testing_env!(context.clone());
        context.attached_deposit = 1000 * STORAGE_PRICE_PER_BYTE;
        testing_env!(context.clone());

        contract.withdraw_from(carol(), bob(), (deposit_amount.clone()+1).into());
    }

    #[test]
    fn withdraw_from_with_correct_allowance_should_be_successful() {
        let mut context = get_context(carol());
        testing_env!(context.clone());

        let mut contract = FungibleToken::new();
        context.storage_usage = env::storage_usage();

        let deposit_amount = 1_000_000_000_000_000u128;
        context.attached_deposit = deposit_amount.clone() + (1000 * STORAGE_PRICE_PER_BYTE);
        testing_env!(context.clone());

        //assert_eq!(contract.get_near_balance().0, 0);

        // get some wNear for carol
        contract.deposit(deposit_amount.clone().into());

        //assert_eq!(contract.get_near_balance().0, 0);

        // TODO: check contract balance == deposit amount
        assert_eq!(contract.get_balance(carol()).0, deposit_amount);
        assert_eq!(contract.get_total_supply().0, deposit_amount);

        // give bob allowance to withdraw some tokens
        assert_eq!(contract.get_allowance(carol(), bob()), ZERO_U128.into());

        let withdrawal_amount = deposit_amount.clone() / 2;
        contract.inc_allowance(bob(), withdrawal_amount.clone().into());

        assert_eq!(contract.get_allowance(carol(), bob()), withdrawal_amount.clone().into());

        // switch to a context with bob
        let mut context = get_context(bob());
        testing_env!(context.clone());
        context.attached_deposit = 1000 * STORAGE_PRICE_PER_BYTE;
        testing_env!(context.clone());

        assert_eq!(contract.get_balance(bob()).0, ZERO_U128.into());

        contract.withdraw_from(carol(), bob(), withdrawal_amount.clone().into());

        //todo: check bob's near balance
        assert_eq!(contract.get_balance(bob()).0, ZERO_U128.into());
        assert_eq!(contract.get_allowance(carol(), bob()), ZERO_U128.into());
        assert_eq!(contract.get_balance(carol()).0, withdrawal_amount.clone().into());
        assert_eq!(contract.get_total_supply().0, withdrawal_amount.clone().into());
    }

    #[test]
    fn withdraw_from_with_infinite_allowance_should_not_reduce_allowance() {
        let mut context = get_context(carol());
        testing_env!(context.clone());

        let mut contract = FungibleToken::new();
        context.storage_usage = env::storage_usage();

        let deposit_amount = 1_000_000_000_000_000u128;
        context.attached_deposit = deposit_amount.clone() + (1000 * STORAGE_PRICE_PER_BYTE);
        testing_env!(context.clone());

        //assert_eq!(contract.get_near_balance().0, 0);

        // get some wNear for carol
        contract.deposit(deposit_amount.clone().into());

        //assert_eq!(contract.get_near_balance().0, 0);

        // TODO: check contract balance == deposit amount
        assert_eq!(contract.get_balance(carol()).0, deposit_amount);
        assert_eq!(contract.get_total_supply().0, deposit_amount);

        // give bob allowance to withdraw some tokens
        assert_eq!(contract.get_allowance(carol(), bob()), ZERO_U128.into());

        let withdrawal_amount = deposit_amount.clone() / 2;
        contract.inc_allowance(bob(), std::u128::MAX.into());

        assert_eq!(contract.get_allowance(carol(), bob()), std::u128::MAX.into());

        // switch to a context with bob
        let mut context = get_context(bob());
        testing_env!(context.clone());
        context.attached_deposit = 1000 * STORAGE_PRICE_PER_BYTE;
        testing_env!(context.clone());

        assert_eq!(contract.get_balance(bob()).0, ZERO_U128.into());

        contract.withdraw_from(carol(), bob(), withdrawal_amount.clone().into());

        //todo: check bob's near balance
        assert_eq!(contract.get_balance(bob()).0, ZERO_U128.into());
        assert_eq!(contract.get_allowance(carol(), bob()), std::u128::MAX.into());
        assert_eq!(contract.get_balance(carol()).0, withdrawal_amount.clone().into());
        assert_eq!(contract.get_total_supply().0, withdrawal_amount.clone().into());
    }

    #[test]
    fn transfer_with_infinite_allowance_should_not_reduce_allowance() {
        let mut context = get_context(carol());
        testing_env!(context.clone());

        let mut contract = FungibleToken::new();
        context.storage_usage = env::storage_usage();

        let deposit_amount = 1_000_000_000_000_000u128;
        context.attached_deposit = deposit_amount.clone() + (1000 * STORAGE_PRICE_PER_BYTE);
        testing_env!(context.clone());

        //assert_eq!(contract.get_near_balance().0, 0);

        // get some wNear for carol
        contract.deposit(deposit_amount.clone().into());

        //assert_eq!(contract.get_near_balance().0, 0);

        // TODO: check contract balance == deposit amount
        assert_eq!(contract.get_balance(carol()).0, deposit_amount);
        assert_eq!(contract.get_total_supply().0, deposit_amount);

        // give bob allowance to withdraw some tokens
        assert_eq!(contract.get_allowance(carol(), bob()), ZERO_U128.into());

        contract.inc_allowance(bob(), std::u128::MAX.into());

        assert_eq!(contract.get_allowance(carol(), bob()), std::u128::MAX.into());

        // switch to a context with bob
        let mut context = get_context(bob());
        testing_env!(context.clone());
        context.attached_deposit = 1000 * STORAGE_PRICE_PER_BYTE;
        testing_env!(context.clone());

        assert_eq!(contract.get_balance(bob()).0, ZERO_U128.into());

        let transfer_amount = deposit_amount.clone() / 2;
        contract.transfer_from(carol(), bob(), transfer_amount.clone().into());

        assert_eq!(contract.get_balance(bob()).0, transfer_amount.clone().into());
        assert_eq!(contract.get_allowance(carol(), bob()), std::u128::MAX.into());
        assert_eq!(contract.get_balance(carol()).0, transfer_amount.clone().into());
        assert_eq!(contract.get_total_supply().0, deposit_amount.clone().into());
    }

    #[test]
    #[should_panic(expected = "Burning more than the account balance")]
    fn withdraw_from_with_infinite_allowance_should_not_withdraw_more_than_balance() {
        let mut context = get_context(carol());
        testing_env!(context.clone());

        let mut contract = FungibleToken::new();
        context.storage_usage = env::storage_usage();

        let deposit_amount = 1_000_000_000_000_000u128;
        context.attached_deposit = deposit_amount.clone() + (1000 * STORAGE_PRICE_PER_BYTE);
        testing_env!(context.clone());

        //assert_eq!(contract.get_near_balance().0, 0);

        // get some wNear for carol
        contract.deposit(deposit_amount.clone().into());

        //assert_eq!(contract.get_near_balance().0, 0);

        // TODO: check contract balance == deposit amount
        assert_eq!(contract.get_balance(carol()).0, deposit_amount);
        assert_eq!(contract.get_total_supply().0, deposit_amount);

        // give bob allowance to withdraw some tokens
        assert_eq!(contract.get_allowance(carol(), bob()), ZERO_U128.into());

        contract.inc_allowance(bob(), std::u128::MAX.into());

        assert_eq!(contract.get_allowance(carol(), bob()), std::u128::MAX.into());

        // switch to a context with bob
        let mut context = get_context(bob());
        testing_env!(context.clone());
        context.attached_deposit = 1000 * STORAGE_PRICE_PER_BYTE;
        testing_env!(context.clone());

        assert_eq!(contract.get_balance(bob()).0, ZERO_U128.into());

        contract.withdraw_from(carol(), bob(), deposit_amount.clone().into());
        contract.withdraw_from(carol(), bob(), deposit_amount.clone().into());
    }

    #[test]
    fn transfer_after_deposit() {
        let mut context = get_context(carol());
        testing_env!(context.clone());
        let mut contract = FungibleToken::new();
        context.storage_usage = env::storage_usage();

        let deposit_amount = 1_000_000_000_000_000u128;
        context.attached_deposit = deposit_amount + (1000 * STORAGE_PRICE_PER_BYTE);
        testing_env!(context.clone());

        // get some wNear tokens
        contract.deposit(deposit_amount.into());

        let transfer_amount = 1_000_000_000_000_000u128 / 3;
        contract.transfer(bob(), transfer_amount.into());
        context.storage_usage = env::storage_usage();
        context.account_balance = env::account_balance();

        context.is_view = true;
        context.attached_deposit = 0;
        testing_env!(context.clone());
        assert_eq!(contract.get_balance(carol()).0, (1_000_000_000_000_000u128 - transfer_amount));
        assert_eq!(contract.get_balance(bob()).0, transfer_amount);
    }

    #[test]
    #[should_panic(expected = "The new owner should be different from the current owner")]
    fn transfer_fail_self() {
        let mut context = get_context(carol());
        testing_env!(context.clone());
        let mut contract = FungibleToken::new();
        context.storage_usage = env::storage_usage();

        let deposit_amount = 1_000_000_000_000_000u128;
        context.attached_deposit = deposit_amount + (1000 * STORAGE_PRICE_PER_BYTE);
        testing_env!(context.clone());

        // get some wNear tokens
        contract.deposit(deposit_amount.into());

        let transfer_amount = deposit_amount.clone() / 2;
        contract.transfer(carol(), transfer_amount.into());
    }

    #[test]
    #[should_panic(expected = "Invalid transfer to this contract")]
    fn transfer_fail_to_contract() {
        let mut context = get_context(carol());
        testing_env!(context.clone());
        let mut contract = FungibleToken::new();
        context.storage_usage = env::storage_usage();

        let deposit_amount = 1_000_000_000_000_000u128;
        context.attached_deposit = deposit_amount + (1000 * STORAGE_PRICE_PER_BYTE);
        testing_env!(context.clone());

        // get some wNear tokens
        contract.deposit(deposit_amount.into());

        let transfer_amount = deposit_amount.clone() / 2;
        contract.transfer(w_near(), transfer_amount.into());
    }

    #[test]
    #[should_panic(expected = "Can not increment allowance for yourself")]
    fn self_inc_allowance_fail() {
        let mut context = get_context(carol());
        testing_env!(context.clone());

        let mut contract = FungibleToken::new();

        context.attached_deposit = 1000 * STORAGE_PRICE_PER_BYTE;
        testing_env!(context.clone());

        contract.inc_allowance(carol(), (5).into());
    }

    #[test]
    #[should_panic(expected = "Can not decrement allowance for yourself")]
    fn self_dec_allowance_fail() {
        let mut context = get_context(carol());
        testing_env!(context.clone());
        let mut contract = FungibleToken::new();
        context.attached_deposit = 1000 * STORAGE_PRICE_PER_BYTE;
        testing_env!(context.clone());
        contract.dec_allowance(carol(), (10).into());
    }

    #[test]
    fn saturating_dec_allowance() {
        let mut context = get_context(carol());
        testing_env!(context.clone());
        let mut contract = FungibleToken::new();
        context.attached_deposit = STORAGE_PRICE_PER_BYTE * 1000;
        testing_env!(context.clone());
        contract.dec_allowance(bob(), (1_000_000_000_000_000u128 / 2).into());
        assert_eq!(contract.get_allowance(carol(), bob()), 0.into())
    }

    #[test]
    fn saturating_inc_allowance() {
        let mut context = get_context(carol());
        testing_env!(context.clone());
        let mut contract = FungibleToken::new();
        context.attached_deposit = STORAGE_PRICE_PER_BYTE * 1000;
        testing_env!(context.clone());
        let max_u128 = std::u128::MAX;
        contract.inc_allowance(bob(), max_u128.into());
        contract.inc_allowance(bob(), max_u128.into());
        assert_eq!(contract.get_allowance(carol(), bob()), std::u128::MAX.into())
    }

    #[test]
    #[should_panic(
    expected = "The required attached deposit is 25700000000000000000000, but the given attached deposit is is 0"
    )]
    fn self_allowance_fail_no_deposit() {
        let mut context = get_context(carol());
        testing_env!(context.clone());
        let mut contract = FungibleToken::new();
        context.attached_deposit = 0;
        testing_env!(context.clone());
        contract.inc_allowance(bob(), 5.into());
    }

    #[test]
    fn carol_escrows_to_bob_transfers_to_alice() {
        // Acting as carol
        let mut context = get_context(carol());
        testing_env!(context.clone());
        let mut contract = FungibleToken::new();
        context.storage_usage = env::storage_usage();

        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_total_supply().0, 0);

        let deposit_amount = 1_000_000_000_000_000u128;
        let allowance = deposit_amount.clone() / 3;
        let transfer_amount = allowance / 3;
        context.is_view = false;

        context.attached_deposit = deposit_amount + (1000 * STORAGE_PRICE_PER_BYTE);
        testing_env!(context.clone());

        // get some wNear tokens
        contract.deposit(deposit_amount.into());

        contract.inc_allowance(bob(), allowance.into());
        context.storage_usage = env::storage_usage();
        context.account_balance = env::account_balance();

        context.is_view = true;
        context.attached_deposit = 0;
        testing_env!(context.clone());
        assert_eq!(contract.get_allowance(carol(), bob()).0, allowance);

        // Acting as bob now
        context.is_view = false;
        context.attached_deposit = STORAGE_PRICE_PER_BYTE * 1000;
        context.predecessor_account_id = bob();
        testing_env!(context.clone());
        contract.transfer_from(carol(), alice(), transfer_amount.into());
        context.storage_usage = env::storage_usage();
        context.account_balance = env::account_balance();

        context.is_view = true;
        context.attached_deposit = 0;
        testing_env!(context.clone());
        assert_eq!(contract.get_balance(carol()).0, deposit_amount.clone() - transfer_amount);
        assert_eq!(contract.get_balance(alice()).0, transfer_amount);
        assert_eq!(contract.get_allowance(carol(), bob()).0, allowance - transfer_amount);
    }

    #[test]
    fn self_allowance_set_for_refund() {
        let mut context = get_context(carol());
        testing_env!(context.clone());
        let mut contract = FungibleToken::new();
        context.storage_usage = env::storage_usage();

        let initial_balance = context.account_balance;
        let initial_storage = context.storage_usage;
        context.attached_deposit = STORAGE_PRICE_PER_BYTE * 1000;
        testing_env!(context.clone());
        let allowance_amount = 1_000_000_000_000_000u128;
        contract.inc_allowance(bob(), (allowance_amount.clone() / 2).into());
        context.storage_usage = env::storage_usage();
        context.account_balance = env::account_balance();
        assert_eq!(
            context.account_balance,
            initial_balance
                + Balance::from(context.storage_usage - initial_storage) * STORAGE_PRICE_PER_BYTE
        );

        let initial_balance = context.account_balance;
        let initial_storage = context.storage_usage;
        testing_env!(context.clone());
        context.attached_deposit = 0;
        testing_env!(context.clone());
        contract.dec_allowance(bob(), (allowance_amount.clone() / 2).into());
        context.storage_usage = env::storage_usage();
        context.account_balance = env::account_balance();
        assert!(context.storage_usage < initial_storage);
        assert!(context.account_balance < initial_balance);
        assert_eq!(
            context.account_balance,
            initial_balance
                - Balance::from(initial_storage - context.storage_usage) * STORAGE_PRICE_PER_BYTE
        );
    }

    #[test]
    fn carol_escrows_to_bob_locks_and_transfers_to_alice() {
        // Acting as carol
        let mut context = get_context(carol());
        testing_env!(context.clone());
        let mut contract = FungibleToken::new();
        context.storage_usage = env::storage_usage();

        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_total_supply().0, 0);

        let deposit_amount = 1_000_000_000_000_000u128;
        let allowance = deposit_amount.clone() / 3;
        let transfer_amount = allowance / 3;
        context.is_view = false;

        context.attached_deposit = deposit_amount + (1000 * STORAGE_PRICE_PER_BYTE);
        testing_env!(context.clone());

        // get some wNear tokens
        contract.deposit(deposit_amount.into());

        contract.inc_allowance(bob(), allowance.into());
        context.storage_usage = env::storage_usage();
        context.account_balance = env::account_balance();

        context.is_view = true;
        context.attached_deposit = 0;
        testing_env!(context.clone());
        assert_eq!(contract.get_allowance(carol(), bob()).0, allowance);
        assert_eq!(contract.get_balance(carol()).0, deposit_amount.clone());

        // Acting as bob now
        context.is_view = false;
        context.attached_deposit = STORAGE_PRICE_PER_BYTE * 1000;
        context.predecessor_account_id = bob();
        testing_env!(context.clone());
        contract.transfer_from(carol(), alice(), transfer_amount.into());
        context.storage_usage = env::storage_usage();
        context.account_balance = env::account_balance();

        context.is_view = true;
        context.attached_deposit = 0;
        testing_env!(context.clone());
        assert_eq!(contract.get_balance(carol()).0, (deposit_amount.clone() - transfer_amount));
        assert_eq!(contract.get_balance(alice()).0, transfer_amount);
        assert_eq!(contract.get_allowance(carol(), bob()).0, allowance - transfer_amount);
    }
}
