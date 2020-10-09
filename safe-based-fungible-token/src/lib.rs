use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::LookupMap;
use near_sdk::json_types::{ValidAccountId, U128};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, ext_contract, near_bindgen, AccountId, Balance, Gas, Promise};

#[global_allocator]
static ALLOC: near_sdk::wee_alloc::WeeAlloc<'_> = near_sdk::wee_alloc::WeeAlloc::INIT;

/// Don't need deposits for function calls.
const NO_DEPOSIT: Balance = 0;

// TODO: Figure out whether we charge for storage or not.
// /// Price per 1 byte of storage from mainnet genesis config.
// const STORAGE_PRICE_PER_BYTE: Balance = 100_000_000_000_000_000_000;

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
        ft.deposit_to_account(owner_id.as_ref(), total_supply);
        ft
    }

    /// Simple transfers
    /// Gas requirement: 5 TGas or 5000000000000 Gas
    /// Should be called by the balance owner.
    ///
    /// Actions:
    /// - Transfers `amount` of tokens from `predecessor_id` to `receiver_id`.
    pub fn transfer_unsafe(&mut self, receiver_id: ValidAccountId, amount: U128) {
        // TODO: Figure out whether we charge for storage or not.
        // let initial_storage = env::storage_usage();

        let amount = amount.into();
        let _sender_id = self.withdraw_from_sender(receiver_id.as_ref(), amount);

        self.deposit_to_account(receiver_id.as_ref(), amount);

        // TODO: Figure out whether we charge for storage or not.
        // self.refund_storage(initial_storage);
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
    /// - Attaches a self callback to this promise `resolve_safe(safe_id, original_amount: amount)`
    pub fn transfer_with_safe(
        &mut self,
        receiver_id: ValidAccountId,
        amount: U128,
        payload: String,
    ) -> Promise {
        // TODO: Figure out whether we charge for storage or not.
        // let initial_storage = env::storage_usage();

        let gas_to_receiver =
            env::prepaid_gas().saturating_sub(GAS_FOR_REMAINING_COMPUTE + GAS_FOR_CALLBACK);

        if gas_to_receiver < MIN_GAS_FOR_RECEIVER {
            env::panic(b"Not enough gas attached. Attach at least 40 TGas");
        }

        let amount = amount.into();
        let sender_id = self.withdraw_from_sender(receiver_id.as_ref(), amount);

        if sender_id == env::current_account_id() {
            env::panic(b"Can't transfer to this token contract");
        }

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

        // TODO: Figure out whether we charge for storage or not.
        // self.refund_storage(initial_storage);
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
        // TODO: Figure out whether we charge for storage or not.
        // let initial_storage = env::storage_usage();

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

        self.deposit_to_account(receiver_id.as_ref(), amount)

        // TODO: Figure out whether we charge for storage or not.
        // self.refund_storage(initial_storage);
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
    /// - Returns the total withdrawn amount from the safe `original_amount - safe.amount`.
    /// #[private]
    pub fn resolve_safe(&mut self, safe_id: SafeId, sender_id: AccountId) -> U128 {
        if env::current_account_id() != env::predecessor_account_id() {
            env::panic(b"This method is private");
        }

        let safe = self.safes.remove(&safe_id).expect("Safe doesn't exist");
        self.deposit_to_account(&sender_id, safe.balance);

        safe.balance.into()
    }

    /// Returns total supply of tokens.
    pub fn get_total_supply(&self) -> U128 {
        self.total_supply.into()
    }

    /// Returns balance of the `owner_id` account.
    pub fn get_balance(&self, account_id: ValidAccountId) -> U128 {
        self.get_account(account_id.as_ref()).0.balance.into()
    }
}

impl SafeBasedFungibleToken {
    /// Withdraws `amount` from the `predecessor_id` while comparing it to the `receiver_id`.
    /// Return `predecessor_id`
    fn withdraw_from_sender(&mut self, receiver_id: &AccountId, amount: Balance) -> AccountId {
        if amount == 0 {
            env::panic(b"Transfer amount should be positive");
        }
        let sender_id = env::predecessor_account_id();
        if &sender_id != receiver_id {
            env::panic(b"The receiver should be different from the sender");
        }
        // Retrieving the account from the state.
        let (mut account, short_account_id) = self.get_account(&sender_id);

        // Checking and updating the balance
        if account.balance < amount {
            env::panic(b"Not enough balance");
        }
        account.balance -= amount;

        // Saving the account back to the state.
        self.set_account(&short_account_id, &account);

        sender_id
    }

    /// Deposits `amount` to the `account_id`
    fn deposit_to_account(&mut self, account_id: &AccountId, amount: Balance) {
        if amount == 0 {
            return;
        }
        // Retrieving the account from the state.
        let (mut account, short_account_id) = self.get_account(&account_id);
        account.balance += amount;

        // Saving the account back to the state.
        self.set_account(&short_account_id, &account);
    }

    /// Helper method to get the account details for `owner_id`.
    fn get_account(&self, account_id: &AccountId) -> (Account, ShortAccountHash) {
        let account_id_hash: ShortAccountHash = account_id.into();
        (
            self.accounts.get(&account_id_hash).unwrap_or_default(),
            account_id_hash,
        )
    }

    /// Helper method to set the account details for `owner_id` to the state.
    fn set_account(&mut self, account_id_hash: &ShortAccountHash, account: &Account) {
        if account.balance > 0 {
            self.accounts.insert(account_id_hash, account);
        } else {
            self.accounts.remove(account_id_hash);
        }
    }

    /*
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
     */
}

// TODO: Add unit test
/*

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use near_sdk::MockedBlockchain;
    use near_sdk::{testing_env, VMContext};

    use super::*;

    fn alice() -> AccountId {
        "alice.near".to_string()
    }
    fn bob() -> AccountId {
        "bob.near".to_string()
    }
    fn carol() -> AccountId {
        "carol.near".to_string()
    }

    fn get_context(predecessor_account_id: AccountId) -> VMContext {
        VMContext {
            current_account_id: alice(),
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
    fn test_new() {
        let context = get_context(carol());
        testing_env!(context);
        let total_supply = 1_000_000_000_000_000u128;
        let contract = FungibleToken::new(bob(), total_supply.into());
        assert_eq!(contract.get_total_supply().0, total_supply);
        assert_eq!(contract.get_balance(bob()).0, total_supply);
    }

    #[test]
    #[should_panic(expected = "The contract is not initialized")]
    fn test_default() {
        let context = get_context(carol());
        testing_env!(context);
        let _contract = FungibleToken::default();
    }

    #[test]
    fn test_transfer() {
        let mut context = get_context(carol());
        testing_env!(context.clone());
        let total_supply = 1_000_000_000_000_000u128;
        let mut contract = FungibleToken::new(carol(), total_supply.into());
        context.storage_usage = env::storage_usage();

        context.attached_deposit = 1000 * STORAGE_PRICE_PER_BYTE;
        testing_env!(context.clone());
        let transfer_amount = total_supply / 3;
        contract.transfer(bob(), transfer_amount.into());
        context.storage_usage = env::storage_usage();
        context.account_balance = env::account_balance();

        context.is_view = true;
        context.attached_deposit = 0;
        testing_env!(context.clone());
        assert_eq!(
            contract.get_balance(carol()).0,
            (total_supply - transfer_amount)
        );
        assert_eq!(contract.get_balance(bob()).0, transfer_amount);
    }

    #[test]
    #[should_panic(expected = "The new owner should be different from the current owner")]
    fn test_transfer_fail_self() {
        let mut context = get_context(carol());
        testing_env!(context.clone());
        let total_supply = 1_000_000_000_000_000u128;
        let mut contract = FungibleToken::new(carol(), total_supply.into());
        context.storage_usage = env::storage_usage();

        context.attached_deposit = 1000 * STORAGE_PRICE_PER_BYTE;
        testing_env!(context.clone());
        let transfer_amount = total_supply / 3;
        contract.transfer(carol(), transfer_amount.into());
    }

    #[test]
    #[should_panic(expected = "Can not increment allowance for yourself")]
    fn test_self_inc_allowance_fail() {
        let mut context = get_context(carol());
        testing_env!(context.clone());
        let total_supply = 1_000_000_000_000_000u128;
        let mut contract = FungibleToken::new(carol(), total_supply.into());
        context.attached_deposit = STORAGE_PRICE_PER_BYTE * 1000;
        testing_env!(context.clone());
        contract.inc_allowance(carol(), (total_supply / 2).into());
    }

    #[test]
    #[should_panic(expected = "Can not decrement allowance for yourself")]
    fn test_self_dec_allowance_fail() {
        let mut context = get_context(carol());
        testing_env!(context.clone());
        let total_supply = 1_000_000_000_000_000u128;
        let mut contract = FungibleToken::new(carol(), total_supply.into());
        context.attached_deposit = STORAGE_PRICE_PER_BYTE * 1000;
        testing_env!(context.clone());
        contract.dec_allowance(carol(), (total_supply / 2).into());
    }

    #[test]
    fn test_saturating_dec_allowance() {
        let mut context = get_context(carol());
        testing_env!(context.clone());
        let total_supply = 1_000_000_000_000_000u128;
        let mut contract = FungibleToken::new(carol(), total_supply.into());
        context.attached_deposit = STORAGE_PRICE_PER_BYTE * 1000;
        testing_env!(context.clone());
        contract.dec_allowance(bob(), (total_supply / 2).into());
        assert_eq!(contract.get_allowance(carol(), bob()), 0.into())
    }

    #[test]
    fn test_saturating_inc_allowance() {
        let mut context = get_context(carol());
        testing_env!(context.clone());
        let total_supply = std::u128::MAX;
        let mut contract = FungibleToken::new(carol(), total_supply.into());
        context.attached_deposit = STORAGE_PRICE_PER_BYTE * 1000;
        testing_env!(context.clone());
        contract.inc_allowance(bob(), total_supply.into());
        contract.inc_allowance(bob(), total_supply.into());
        assert_eq!(
            contract.get_allowance(carol(), bob()),
            std::u128::MAX.into()
        )
    }

    #[test]
    #[should_panic(
        expected = "The required attached deposit is 12400000000000000000000, but the given attached deposit is is 0"
    )]
    fn test_self_allowance_fail_no_deposit() {
        let mut context = get_context(carol());
        testing_env!(context.clone());
        let total_supply = 1_000_000_000_000_000u128;
        let mut contract = FungibleToken::new(carol(), total_supply.into());
        context.attached_deposit = 0;
        testing_env!(context.clone());
        contract.inc_allowance(bob(), (total_supply / 2).into());
    }

    #[test]
    fn test_carol_escrows_to_bob_transfers_to_alice() {
        // Acting as carol
        let mut context = get_context(carol());
        testing_env!(context.clone());
        let total_supply = 1_000_000_000_000_000u128;
        let mut contract = FungibleToken::new(carol(), total_supply.into());
        context.storage_usage = env::storage_usage();

        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_total_supply().0, total_supply);

        let allowance = total_supply / 3;
        let transfer_amount = allowance / 3;
        context.is_view = false;
        context.attached_deposit = STORAGE_PRICE_PER_BYTE * 1000;
        testing_env!(context.clone());
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
        assert_eq!(
            contract.get_balance(carol()).0,
            total_supply - transfer_amount
        );
        assert_eq!(contract.get_balance(alice()).0, transfer_amount);
        assert_eq!(
            contract.get_allowance(carol(), bob()).0,
            allowance - transfer_amount
        );
    }

    #[test]
    fn test_carol_escrows_to_bob_locks_and_transfers_to_alice() {
        // Acting as carol
        let mut context = get_context(carol());
        testing_env!(context.clone());
        let total_supply = 1_000_000_000_000_000u128;
        let mut contract = FungibleToken::new(carol(), total_supply.into());
        context.storage_usage = env::storage_usage();

        context.is_view = true;
        testing_env!(context.clone());
        assert_eq!(contract.get_total_supply().0, total_supply);

        let allowance = total_supply / 3;
        let transfer_amount = allowance / 3;
        context.is_view = false;
        context.attached_deposit = STORAGE_PRICE_PER_BYTE * 1000;
        testing_env!(context.clone());
        contract.inc_allowance(bob(), allowance.into());
        context.storage_usage = env::storage_usage();
        context.account_balance = env::account_balance();

        context.is_view = true;
        context.attached_deposit = 0;
        testing_env!(context.clone());
        assert_eq!(contract.get_allowance(carol(), bob()).0, allowance);
        assert_eq!(contract.get_balance(carol()).0, total_supply);

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
        assert_eq!(
            contract.get_balance(carol()).0,
            (total_supply - transfer_amount)
        );
        assert_eq!(contract.get_balance(alice()).0, transfer_amount);
        assert_eq!(
            contract.get_allowance(carol(), bob()).0,
            allowance - transfer_amount
        );
    }

    #[test]
    fn test_self_allowance_set_for_refund() {
        let mut context = get_context(carol());
        testing_env!(context.clone());
        let total_supply = 1_000_000_000_000_000u128;
        let mut contract = FungibleToken::new(carol(), total_supply.into());
        context.storage_usage = env::storage_usage();

        let initial_balance = context.account_balance;
        let initial_storage = context.storage_usage;
        context.attached_deposit = STORAGE_PRICE_PER_BYTE * 1000;
        testing_env!(context.clone());
        contract.inc_allowance(bob(), (total_supply / 2).into());
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
        contract.dec_allowance(bob(), (total_supply / 2).into());
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
}


*/
