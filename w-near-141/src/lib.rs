/**
* wNear NEP-141 Token contract
*
* The aim of the contract is to enable the wrapping of the native NEAR token into a NEP-141 compatible token.
* It supports methods `near_deposit` and `near_withdraw` that wraps and unwraps NEAR tokens.
* They are effectively mint and burn underlying wNEAR tokens.
*
* lib.rs is the main entry point.
* fungible_token_core.rs implements NEP-146 standard
* storage_manager.rs implements NEP-145 standard for allocating storage per account
* fungible_token_metadata.rs implements NEP-148 standard for providing token-specific metadata.
* w_near.rs contains interfaces for depositing and withdrawing
* internal.rs contains internal methods for fungible token.
*/
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::LookupMap;
use near_sdk::json_types::U128;
use near_sdk::{env, near_bindgen, AccountId, Balance, Promise, StorageUsage};

pub use crate::fungible_token_core::*;
pub use crate::fungible_token_metadata::*;
use crate::internal::*;
pub use crate::storage_manager::*;
pub use crate::w_near::*;

mod fungible_token_core;
mod fungible_token_metadata;
mod internal;
mod storage_manager;
mod w_near;

#[global_allocator]
static ALLOC: near_sdk::wee_alloc::WeeAlloc<'_> = near_sdk::wee_alloc::WeeAlloc::INIT;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct Contract {
    /// AccountID -> Account balance.
    pub accounts: LookupMap<AccountId, Balance>,

    /// Total supply of the all token.
    pub total_supply: Balance,

    /// The storage size in bytes for one account.
    pub account_storage_usage: StorageUsage,
}

impl Default for Contract {
    fn default() -> Self {
        env::panic(b"Contract is not initialized");
    }
}

#[near_bindgen]
impl Contract {
    #[init]
    pub fn new() -> Self {
        assert!(!env::state_exists(), "Already initialized");
        let mut this = Self {
            accounts: LookupMap::new(b"a".to_vec()),
            total_supply: 0,
            account_storage_usage: 0,
        };
        let initial_storage_usage = env::storage_usage();
        let tmp_account_id = unsafe { String::from_utf8_unchecked(vec![b'a'; 64]) };
        this.accounts.insert(&tmp_account_id, &0u128);
        this.account_storage_usage = env::storage_usage() - initial_storage_usage;
        this.accounts.remove(&tmp_account_id);
        this
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
    #[should_panic(
        expected = "The required attached deposit is 13300001000000000000000, but the given attached deposit is is 13300000000000000000000"
    )]
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

        contract.withdraw((deposit_amount.clone() + 1).into());
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

        assert_eq!(
            contract.get_allowance(carol(), bob()),
            std::u128::MAX.into()
        );

        // switch to a context with bob
        let mut context = get_context(bob());
        testing_env!(context.clone());
        context.attached_deposit = 1000 * STORAGE_PRICE_PER_BYTE;
        testing_env!(context.clone());

        contract.withdraw_from(carol(), bob(), (deposit_amount.clone() + 1).into());
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

        assert_eq!(
            contract.get_allowance(carol(), bob()),
            withdrawal_amount.clone().into()
        );

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
        assert_eq!(
            contract.get_balance(carol()).0,
            withdrawal_amount.clone().into()
        );
        assert_eq!(
            contract.get_total_supply().0,
            withdrawal_amount.clone().into()
        );
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

        assert_eq!(
            contract.get_allowance(carol(), bob()),
            std::u128::MAX.into()
        );

        // switch to a context with bob
        let mut context = get_context(bob());
        testing_env!(context.clone());
        context.attached_deposit = 1000 * STORAGE_PRICE_PER_BYTE;
        testing_env!(context.clone());

        assert_eq!(contract.get_balance(bob()).0, ZERO_U128.into());

        contract.withdraw_from(carol(), bob(), withdrawal_amount.clone().into());

        //todo: check bob's near balance
        assert_eq!(contract.get_balance(bob()).0, ZERO_U128.into());
        assert_eq!(
            contract.get_allowance(carol(), bob()),
            std::u128::MAX.into()
        );
        assert_eq!(
            contract.get_balance(carol()).0,
            withdrawal_amount.clone().into()
        );
        assert_eq!(
            contract.get_total_supply().0,
            withdrawal_amount.clone().into()
        );
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

        assert_eq!(
            contract.get_allowance(carol(), bob()),
            std::u128::MAX.into()
        );

        // switch to a context with bob
        let mut context = get_context(bob());
        testing_env!(context.clone());
        context.attached_deposit = 1000 * STORAGE_PRICE_PER_BYTE;
        testing_env!(context.clone());

        assert_eq!(contract.get_balance(bob()).0, ZERO_U128.into());

        let transfer_amount = deposit_amount.clone() / 2;
        contract.transfer_from(carol(), bob(), transfer_amount.clone().into());

        assert_eq!(
            contract.get_balance(bob()).0,
            transfer_amount.clone().into()
        );
        assert_eq!(
            contract.get_allowance(carol(), bob()),
            std::u128::MAX.into()
        );
        assert_eq!(
            contract.get_balance(carol()).0,
            transfer_amount.clone().into()
        );
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

        assert_eq!(
            contract.get_allowance(carol(), bob()),
            std::u128::MAX.into()
        );

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
        assert_eq!(
            contract.get_balance(carol()).0,
            (1_000_000_000_000_000u128 - transfer_amount)
        );
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
        assert_eq!(
            contract.get_allowance(carol(), bob()),
            std::u128::MAX.into()
        )
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
        assert_eq!(
            contract.get_balance(carol()).0,
            deposit_amount.clone() - transfer_amount
        );
        assert_eq!(contract.get_balance(alice()).0, transfer_amount);
        assert_eq!(
            contract.get_allowance(carol(), bob()).0,
            allowance - transfer_amount
        );
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
        assert_eq!(
            contract.get_balance(carol()).0,
            (deposit_amount.clone() - transfer_amount)
        );
        assert_eq!(contract.get_balance(alice()).0, transfer_amount);
        assert_eq!(
            contract.get_allowance(carol(), bob()).0,
            allowance - transfer_amount
        );
    }
}
