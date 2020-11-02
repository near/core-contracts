use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::LookupMap;
use near_sdk::json_types::{Base64VecU8, ValidAccountId, U128};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, ext_contract, near_bindgen, AccountId, Balance, Gas, Promise, PromiseResult};
use uint::construct_uint;

#[global_allocator]
static ALLOC: near_sdk::wee_alloc::WeeAlloc<'_> = near_sdk::wee_alloc::WeeAlloc::INIT;

construct_uint! {
    /// 256-bit unsigned integer.
    pub struct U256(4);
}

/// Price per 1 byte of storage from mainnet genesis config.
const STORAGE_PRICE_PER_BYTE: Balance = 100_000_000_000_000_000_000;

/// The amount of NEAR to send to the safe-based fungible token to register this contract.
const SAFE_BASED_ACCOUNT_REGISTRATION_DEPOSIT: Balance = 77 * STORAGE_PRICE_PER_BYTE;

/// Don't need deposits for function calls.
const NO_DEPOSIT: Balance = 0;

/// Standard 0.3%
const FEE_NUMERATOR: u32 = 1003;
const FEE_DENOMINATOR: u32 = 1000;

/// NOTE: These fees are going to change with the update.
/// Basic compute.
const GAS_BASE_COMPUTE: Gas = 5_000_000_000_000;
/// Fee for function call promise.
const GAS_FOR_PROMISE: Gas = 5_000_000_000_000;

/// It needs only base compute if we pass the exact amount.
const GAS_FOR_ACCOUNT_REGISTRATION: Gas = GAS_FOR_PROMISE + GAS_BASE_COMPUTE;

const GAS_FOR_INTERNAL_DEPOSIT: Gas = GAS_BASE_COMPUTE;

const GAS_FOR_WITHDRAW_FROM_SAFE: Gas = GAS_BASE_COMPUTE;
const GAS_FOR_TRANSFER_UNSAFE: Gas = GAS_BASE_COMPUTE;

/// Safe identifier.
#[derive(Serialize, Deserialize, Clone, Copy)]
#[serde(crate = "near_sdk::serde")]
pub struct SafeId(pub u64);

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
    /// Account balance in fungible tokens.
    pub token_balance: Balance,

    /// Account balance in NEAR tokens.
    pub near_balance: Balance,

    /// Account balance of the liquidity token for this Pool.
    pub liquidity_balance: Balance,
}

impl Default for Account {
    fn default() -> Self {
        Self {
            token_balance: 0,
            near_balance: 0,
            liquidity_balance: 0,
        }
    }
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct UniswapPool {
    /// Accounts
    pub accounts: LookupMap<ShortAccountHash, Account>,

    /// The account ID of the fungible token contract
    pub token_account_id: AccountId,

    /// Total fungible token balance locked
    pub total_token_balance: Balance,

    /// Total NEAR token balance locked in yoctoNEAR.
    pub total_near_balance: Balance,

    /// Total balance of the liquidity token for this Pool.
    pub total_liquidity_balance: Balance,
}

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct BalancePair {
    token_balance: U128,

    near_balance: U128,
}

impl Default for UniswapPool {
    fn default() -> Self {
        env::panic(b"The contract is not initialized.");
    }
}

#[ext_contract(ext_token)]
pub trait ExtSafeBasedFungibleToken {
    fn withdraw_from_safe(&mut self, safe_id: SafeId, receiver_id: AccountId, amount: U128);
    fn register_account(&mut self, account_id: AccountId);
    fn transfer_unsafe(&mut self, receiver_id: AccountId, amount: U128);
}

#[ext_contract(ext_self)]
pub trait ExtSelf {
    fn on_withdraw_from_safe_deposit(&mut self, account_id: AccountId, amount: U128);
}

#[derive(BorshSerialize, BorshDeserialize)]
pub enum OnReceiverPayload {
    Deposit,
    SwapForNear { desired_near_amount: Balance },
}

#[near_bindgen]
impl UniswapPool {
    /// Initializes the contract to start a pool for a given fungible token.
    /// The `token_account_id` should have this contract registered as an account.
    #[init]
    pub fn new(token_account_id: ValidAccountId) -> Self {
        assert!(!env::state_exists(), "Already initialized");
        ext_token::register_account(
            env::current_account_id(),
            token_account_id.as_ref(),
            SAFE_BASED_ACCOUNT_REGISTRATION_DEPOSIT,
            GAS_FOR_ACCOUNT_REGISTRATION,
        );
        Self {
            accounts: LookupMap::new(b"a".to_vec()),
            token_account_id: token_account_id.into(),
            total_near_balance: 0,
            total_token_balance: 0,
            total_liquidity_balance: 0,
        }
    }

    #[payable]
    pub fn deposit_near(&mut self) {
        let account_id = env::predecessor_account_id();
        let (mut account, account_id_hash) = self.get_account_expect(&account_id);
        let attached_deposit = env::attached_deposit();
        account.near_balance += attached_deposit;
        self.set_account(&account_id_hash, &account);
    }

    pub fn withdraw_near(&mut self, amount: U128) -> Promise {
        let account_id = env::predecessor_account_id();
        let (mut account, account_id_hash) = self.get_account_expect(&account_id);
        let amount = amount.into();
        if account.near_balance < amount {
            env::panic(b"Not enough liquid NEAR balance");
        }
        account.near_balance -= amount;
        self.set_account(&account_id_hash, &account);

        Promise::new(account_id).transfer(amount)
    }

    pub fn withdraw_token_unsafe(&mut self, amount: U128) -> Promise {
        let account_id = env::predecessor_account_id();
        let (mut account, account_id_hash) = self.get_account_expect(&account_id);
        let amount = amount.into();
        if account.token_balance < amount {
            env::panic(b"Not enough liquid token balance");
        }
        account.token_balance -= amount;
        self.set_account(&account_id_hash, &account);

        ext_token::transfer_unsafe(
            account_id,
            amount.into(),
            &self.token_account_id,
            NO_DEPOSIT,
            GAS_FOR_TRANSFER_UNSAFE,
        )
    }

    pub fn on_receive_with_safe(
        &mut self,
        sender_id: ValidAccountId,
        amount: U128,
        safe_id: SafeId,
        payload: Base64VecU8,
    ) -> Promise {
        if &env::predecessor_account_id() != &self.token_account_id {
            env::panic(b"This pool only works with different fungible token account");
        }
        let payload: OnReceiverPayload =
            BorshDeserialize::try_from_slice(&payload.0).expect("Failed to parse the payload");

        let amount: Balance = amount.into();

        match payload {
            OnReceiverPayload::Deposit => {
                self.assert_account_exists(sender_id.as_ref());
                ext_token::withdraw_from_safe(
                    safe_id,
                    env::current_account_id(),
                    amount.into(),
                    &self.token_account_id,
                    NO_DEPOSIT,
                    GAS_FOR_WITHDRAW_FROM_SAFE,
                )
                .then(ext_self::on_withdraw_from_safe_deposit(
                    sender_id.into(),
                    amount.into(),
                    &env::current_account_id(),
                    NO_DEPOSIT,
                    GAS_FOR_INTERNAL_DEPOSIT,
                ))
            }
            OnReceiverPayload::SwapForNear {
                desired_near_amount,
            } => {
                if self.total_near_balance <= desired_near_amount {
                    env::panic(
                        format!(
                            "Available total NEAR balance {} is less or equal than the desired NEAR amount {}",
                            self.total_near_balance, desired_near_amount
                        )
                        .as_bytes(),
                    );
                }
                // product = self.total_near_balance * self.total_token_balance;
                // product = (self.total_near_balance - desired_near_amount) * (self.total_token_balance + required_token_amount);
                // required_token_amount = product / (self.total_near_balance - desired_near_amount) - self.total_token_balance

                let required_token_amount = U256::from(FEE_NUMERATOR)
                    * U256::from(self.total_token_balance)
                    * U256::from(desired_near_amount)
                    / (U256::from(FEE_DENOMINATOR)
                        * U256::from(self.total_near_balance - desired_near_amount));

                if required_token_amount > U256::from(amount) {
                    env::panic(
                        format!(
                            "Provided token amount {} is less than the required token amount {}",
                            amount, required_token_amount
                        )
                        .as_bytes(),
                    );
                }
                let required_token_amount = required_token_amount.as_u128();
                self.total_token_balance += required_token_amount;
                self.total_near_balance -= desired_near_amount;
                env::log(
                    format!(
                        "Swapped {} tokens for {} NEAR",
                        required_token_amount, desired_near_amount
                    )
                    .as_bytes(),
                );

                // If we schedule a transfer of NEAR in parallel, then it will not be a part of one
                // transaction, so the composability might break.
                // Another option is to attach a callback on both of them, but this is unnecessary.
                ext_token::withdraw_from_safe(
                    safe_id,
                    env::current_account_id(),
                    required_token_amount.into(),
                    &self.token_account_id,
                    NO_DEPOSIT,
                    GAS_FOR_WITHDRAW_FROM_SAFE,
                )
                .then(Promise::new(sender_id.into()).transfer(desired_near_amount))
            }
        }
    }

    #[payable]
    pub fn swap_for_tokens_unsafe(&mut self, desired_token_amount: U128) -> Promise {
        let amount = env::attached_deposit();
        let account_id = env::predecessor_account_id();
        let desired_token_amount: Balance = desired_token_amount.into();

        if self.total_token_balance <= desired_token_amount {
            env::panic(
                format!(
                    "Available total token balance {} is less or equal than the desired token amount {}",
                    self.total_token_balance, desired_token_amount
                )
                    .as_bytes(),
            );
        }
        // product = self.total_token_balance * self.total_near_balance;
        // product = (self.total_token_balance - desired_token_amount) * (self.total_near_balance + required_near_amount);
        // required_near_amount = product / (self.total_token_balance - desired_token_amount) - self.total_near_balance

        let required_near_amount = U256::from(FEE_NUMERATOR)
            * U256::from(self.total_near_balance)
            * U256::from(desired_token_amount)
            / (U256::from(FEE_DENOMINATOR)
                * U256::from(self.total_token_balance - desired_token_amount));

        if required_near_amount > U256::from(amount) {
            env::panic(
                format!(
                    "Provided NEAR amount {} is less than the required NEAR amount {}",
                    amount, required_near_amount
                )
                .as_bytes(),
            );
        }
        let required_near_amount = required_near_amount.as_u128();
        self.total_near_balance += required_near_amount;
        self.total_token_balance -= desired_token_amount;
        env::log(
            format!(
                "Swapped {} NEAR for {} tokens",
                required_near_amount, desired_token_amount
            )
            .as_bytes(),
        );

        let token_transfer_promise = ext_token::transfer_unsafe(
            account_id.clone(),
            desired_token_amount.into(),
            &self.token_account_id,
            NO_DEPOSIT,
            GAS_FOR_WITHDRAW_FROM_SAFE,
        );

        let refund_amount = amount - required_near_amount;
        if refund_amount > 0 {
            token_transfer_promise.then(Promise::new(account_id).transfer(refund_amount))
        } else {
            token_transfer_promise
        }
    }

    /// Callback
    pub fn on_withdraw_from_safe_deposit(&mut self, account_id: AccountId, amount: U128) {
        assert_self();
        if !is_promise_success() {
            env::panic(b"Withdrawal from the safe failed");
        }
        let (mut account, account_id_hash) = self.get_account_expect(&account_id);
        let amount: u128 = amount.into();
        account.token_balance += amount;
        self.set_account(&account_id_hash, &account);
    }

    pub fn buy_liqidity(
        &mut self,
        max_near_amount: U128,
        max_token_amount: U128,
        desired_liquidity_amount: U128,
    ) -> U128 {
        let account_id = env::predecessor_account_id();
        let (mut account, account_id_hash) = self.get_account_expect(&account_id);
        let max_near_amount: Balance = max_near_amount.into();
        let max_token_amount: Balance = max_token_amount.into();
        let desired_liquidity_amount: Balance = desired_liquidity_amount.into();
        if max_near_amount == 0 {
            env::panic(b"Max token amount should be positive");
        }
        if max_token_amount == 0 {
            env::panic(b"Max NEAR amount should be positive");
        }
        if self.total_liquidity_balance == 0 && desired_liquidity_amount == 0 {
            env::panic(b"Desired liquidity token amount should be positive");
        }
        if max_token_amount > account.token_balance {
            env::panic(
                format!(
                    "Max token amount {} should be less or equal to the account token balance {}",
                    max_token_amount, account.token_balance
                )
                .as_bytes(),
            );
        }
        if max_near_amount > account.near_balance {
            env::panic(
                format!(
                    "Max NEAR amount {} should be less or equal to the account NEAR balance {}",
                    max_near_amount, account.near_balance
                )
                .as_bytes(),
            );
        }
        let total_liquidity_balance = U256::from(self.total_liquidity_balance);
        let total_near_balance = U256::from(self.total_near_balance);
        let total_token_balance = U256::from(self.total_token_balance);

        let (liquidity_amount, near_amount, token_amount) = if self.total_liquidity_balance == 0 {
            // Uninitialized. Minting max amounts.
            let token_amount = max_token_amount;
            let near_amount = max_near_amount;
            let liquidity_amount = std::cmp::min(max_token_amount, max_near_amount);
            (liquidity_amount, near_amount, token_amount)
        } else {
            // token_price = token_balance / liquidity_balance
            // max_token_shares = max_token_amount / token_price = max_token_amount * liquidity_balance / token_balance
            // near_price = near_balance / liquidity_balance
            // max_near_shares = max_near_amount / near_price = max_near_amount * liquidity_balance / near_balance
            let max_liquidity_amount = if U256::from(max_token_amount) * total_near_balance
                > U256::from(max_near_amount) * total_token_balance
            {
                // limited by max NEAR amount
                U256::from(max_near_amount) * total_liquidity_balance / total_near_balance
            } else {
                // limited by max token amount
                U256::from(max_token_amount) * total_liquidity_balance / total_token_balance
            };
            let liquidity_amount = U256::from(desired_liquidity_amount);
            if max_liquidity_amount < liquidity_amount {
                env::panic(
                    format!(
                        "The max liquidity amount {} is less than the desired liquidity amount {}",
                        max_liquidity_amount.as_u128(),
                        desired_liquidity_amount
                    )
                    .as_bytes(),
                );
            }

            let near_amount = (liquidity_amount * total_near_balance + total_liquidity_balance - 1)
                / total_liquidity_balance;
            let token_amount = (liquidity_amount * total_near_balance + total_liquidity_balance
                - 1)
                / total_liquidity_balance;
            (
                desired_liquidity_amount,
                near_amount.as_u128(),
                token_amount.as_u128(),
            )
        };

        account.liquidity_balance += liquidity_amount;
        account.near_balance -= near_amount;
        account.token_balance -= token_balance;

        env::log(
            format!(
                "Bought {} liquidity for token amount {} and NEAR amount {}",
                liquidity_amount, token_amount, token_balance
            )
            .as_bytes(),
        );

        self.total_liquidity_balance += liquidity_amount;
        self.total_near_balance += near_balance;
        self.total_token_balance += token_balance;

        self.set_account(&account_id_hash, &account);

        liquidity_amount.into()
    }

    pub fn sell_liqidity(
        &mut self,
        liquidity_amount: U128,
        min_near_amount: U128,
        min_token_amount: U128,
    ) -> (U128, U128) {
        let account_id = env::predecessor_account_id();
        let (mut account, account_id_hash) = self.get_account_expect(&account_id);
        let min_near_amount: Balance = min_near_amount.into();
        let min_token_amount: Balance = min_token_amount.into();
        let liquidity_amount: Balance = liquidity_amount.into();
        if self.liquidity_amount == 0 {
            env::panic(b"Liquidity amount should be positive");
        }
        if liquidity_amount > account.liquidity_balance {
            env::panic(
                format!(
                    "Liquidity amount {} should be less or equal to the account liquidity balance {}",
                    liquidity_amount, account.liquidity_balance
                )
                .as_bytes(),
            );
        }
        let liquidity_amount_u256 = U256::from(liquidity_amount);
        let total_liquidity_balance = U256::from(self.total_liquidity_balance);

        let near_amount = ((liquidity_amount_u256 * U256::from(self.total_near_balance))
            / total_liquidity_balance)
            .as_u128();
        let token_amount = ((liquidity_amount_u256 * U256::from(self.total_token_balance))
            / total_liquidity_balance)
            .as_u128();

        if near_amount < min_near_amount {
            env::panic(
                format!(
                    "Received NEAR amount {} is less than the desired minimum NEAR amount {}",
                    near_amount, min_near_amount
                )
                .as_bytes(),
            );
        }
        if token_amount < min_token_amount {
            env::panic(
                format!(
                    "Received token amount {} is less than the desired minimum token amount {}",
                    token_amount, min_token_amount
                )
                .as_bytes(),
            );
        }

        account.liquidity_balance -= liquidity_amount;
        account.near_balance += near_amount;
        account.token_balance += token_balance;

        env::log(
            format!(
                "Sold {} liquidity for token amount {} and NEAR amount {}",
                liquidity_amount, token_amount, token_balance
            )
            .as_bytes(),
        );

        self.total_liquidity_balance -= liquidity_amount;
        self.total_near_balance -= near_balance;
        self.total_token_balance -= token_balance;

        self.set_account(&account_id_hash, &account);

        (near_amount.into(), token_amount.into())
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
            if account.near_balance > 0 || account.token_balance > 0 {
                env::panic(
                    format!(
                        "Can't unregister account {}, because it has a positive balance",
                        account_id,
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

    pub fn get_total_balances(&self) -> BalancePair {
        BalancePair {
            token_balance: self.total_token_balance.into(),
            near_balance: self.total_near_balance.into(),
        }
    }

    /// Returns total supply of tokens.
    pub fn get_total_token_balance(&self) -> U128 {
        self.total_token_balance.into()
    }

    /// Returns total supply of tokens.
    pub fn get_total_near_balance(&self) -> U128 {
        self.total_near_balance.into()
    }

    // /// Returns balance of the `owner_id` account.
    // pub fn get_balance(&self, account_id: ValidAccountId) -> U128 {
    //     self.accounts
    //         .get(&account_id.as_ref().into())
    //         .map(|account| account.balance)
    //         .unwrap_or(0)
    //         .into()
    // }
}

fn is_promise_success() -> bool {
    if env::promise_results_count() != 1 {
        env::panic(b"Contract expected a result on the callback");
    }
    match env::promise_result(0) {
        PromiseResult::Successful(_) => true,
        _ => false,
    }
}

fn assert_self() {
    if env::current_account_id() != env::predecessor_account_id() {
        env::panic(b"This method is private");
    }
}

impl UniswapPool {
    fn assert_account_exists(&self, account_id: &AccountId) {
        if !self.accounts.contains_key(&account_id.into()) {
            env::panic(format!("Account {} is not registered", account_id).as_bytes());
        }
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
//
// #[cfg(not(target_arch = "wasm32"))]
// #[cfg(test)]
// mod tests {
//     use near_sdk::{serde_json, MockedBlockchain};
//     use near_sdk::{testing_env, VMContext};
//
//     use super::*;
//     use std::convert::TryInto;
//
//     fn alice() -> ValidAccountId {
//         "alice.near".try_into().unwrap()
//     }
//     fn bob() -> ValidAccountId {
//         "bob.near".try_into().unwrap()
//     }
//     fn carol() -> ValidAccountId {
//         "carol.near".try_into().unwrap()
//     }
//
//     fn get_context(
//         predecessor_account_id: AccountId,
//         is_view: bool,
//         attached_deposit: Balance,
//     ) -> VMContext {
//         VMContext {
//             current_account_id: alice().into(),
//             signer_account_id: bob().into(),
//             signer_account_pk: vec![0, 1, 2],
//             predecessor_account_id,
//             input: vec![],
//             block_index: 0,
//             block_timestamp: 0,
//             account_balance: 1000 * 10u128.pow(24),
//             account_locked_balance: 0,
//             storage_usage: 10u64.pow(6),
//             attached_deposit,
//             prepaid_gas: 10u64.pow(18),
//             random_seed: vec![0, 1, 2],
//             is_view,
//             output_data_receivers: vec![],
//             epoch_height: 0,
//         }
//     }
//
//     fn context(predecessor_account_id: ValidAccountId) {
//         testing_env!(get_context(predecessor_account_id.into(), false, 0));
//     }
//
//     fn view_context() {
//         testing_env!(get_context("view".to_string(), true, 0));
//     }
//
//     fn context_with_deposit(predecessor_account_id: ValidAccountId, attached_deposit: Balance) {
//         testing_env!(get_context(
//             predecessor_account_id.into(),
//             false,
//             attached_deposit
//         ));
//     }
//
//     #[test]
//     fn test_new() {
//         context(carol());
//         let total_supply = 1_000_000_000_000_000u128;
//         let contract = SafeBasedFungibleToken::new(bob(), total_supply.into());
//         view_context();
//         assert_eq!(contract.get_total_supply().0, total_supply);
//         assert_eq!(contract.get_balance(bob()).0, total_supply);
//     }
//
//     #[test]
//     #[should_panic(expected = "The contract is not initialized")]
//     fn test_default() {
//         context(carol());
//         let _contract = SafeBasedFungibleToken::default();
//     }
//
//     #[test]
//     fn test_transfer_unsafe() {
//         context(carol());
//         let total_supply = 1_000_000_000_000_000u128;
//         let mut contract = SafeBasedFungibleToken::new(carol(), total_supply.into());
//
//         context_with_deposit(carol(), 77 * STORAGE_PRICE_PER_BYTE);
//         contract.register_account(bob());
//         // No refunds
//         assert!(env::created_receipts().is_empty());
//
//         context(carol());
//         let transfer_amount = total_supply / 3;
//         contract.transfer_unsafe(bob(), transfer_amount.into());
//
//         view_context();
//         assert_eq!(
//             contract.get_balance(carol()).0,
//             (total_supply - transfer_amount)
//         );
//         assert_eq!(contract.get_balance(bob()).0, transfer_amount);
//     }
//
//     #[test]
//     #[should_panic(expected = "The receiver should be different from the sender")]
//     fn test_transfer_unsafe_fail_self() {
//         context(carol());
//         let total_supply = 1_000_000_000_000_000u128;
//         let mut contract = SafeBasedFungibleToken::new(carol(), total_supply.into());
//
//         context(carol());
//         let transfer_amount = total_supply / 3;
//         contract.transfer_unsafe(carol(), transfer_amount.into());
//     }
//
//     #[test]
//     fn test_transfer_with_safe() {
//         context(carol());
//         let total_supply = 1_000_000_000_000_000u128;
//         let mut contract = SafeBasedFungibleToken::new(carol(), total_supply.into());
//
//         context_with_deposit(bob(), 77 * STORAGE_PRICE_PER_BYTE);
//         contract.register_account(bob());
//         // No refunds
//         assert!(env::created_receipts().is_empty());
//
//         context(carol());
//         let transfer_amount = total_supply / 3;
//         contract.transfer_with_safe(bob(), transfer_amount.into(), "PAYLOAD".to_string());
//
//         assert_eq!(contract.next_safe_id.0, 1);
//
//         let receipts = env::created_receipts();
//
//         assert_eq!(receipts.len(), 2);
//         println!("{}", serde_json::to_string(&receipts[0]).unwrap());
//         println!("{}", serde_json::to_string(&receipts[1]).unwrap());
//
//         // Checking balances. Carol should have less, but bob still doesn't have it
//         view_context();
//         assert_eq!(
//             contract.get_balance(carol()).0,
//             (total_supply - transfer_amount)
//         );
//         assert_eq!(contract.get_balance(bob()).0, 0);
//
//         // Assuming we're bob() and received
//         // `on_receive_with_safe({"sender_id":"carol.near","amount":"333333333333333","safe_id":0,"payload":"PAYLOAD"})`.
//         context(bob());
//         let withdrawal_amount = transfer_amount / 2;
//         contract.withdraw_from_safe(SafeId(0), bob(), withdrawal_amount.into());
//
//         // Checking balances. Carol should have less, but Bob has withdrawal amount
//         view_context();
//         assert_eq!(
//             contract.get_balance(carol()).0,
//             (total_supply - transfer_amount)
//         );
//         assert_eq!(contract.get_balance(bob()).0, withdrawal_amount);
//
//         // Resolving the safe
//         context(alice());
//         let res = contract.resolve_safe(SafeId(0), carol().into());
//         assert_eq!(res.0, transfer_amount - withdrawal_amount);
//
//         // Final balances
//         view_context();
//         assert_eq!(
//             contract.get_balance(carol()).0,
//             (total_supply - withdrawal_amount)
//         );
//         assert_eq!(contract.get_balance(bob()).0, withdrawal_amount);
//     }
// }
