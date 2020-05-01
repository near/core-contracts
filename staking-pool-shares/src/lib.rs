use std::convert::TryInto;

use borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::collections::Map;
use near_sdk::json_types::{Base58PublicKey, U128};
use near_sdk::{env, near_bindgen, AccountId, Balance, EpochHeight, Promise, PublicKey};

use uint::construct_uint;

const PING_GAS: u64 = 30_000_000_000_000;
const INTERNAL_AFTER_STAKE_GAS: u64 = 30_000_000_000_000;

construct_uint! {
    /// 256-bit unsigned integer.
    pub struct U256(4);
}

#[cfg(test)]
mod test_utils;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[derive(BorshDeserialize, BorshSerialize, Debug, PartialEq)]
pub struct Account {
    pub unstaked: Balance,
    pub staked_shares: Balance,
    pub unstaked_available_epoch_height: EpochHeight,
}

impl Default for Account {
    fn default() -> Self {
        Self {
            unstaked: 0,
            staked_shares: 0,
            unstaked_available_epoch_height: 0,
        }
    }
}

const EPOCHS_TOWARDS_REWARD: EpochHeight = 3;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct StakingContract {
    pub owner_id: AccountId,
    pub stake_public_key: PublicKey,
    pub last_epoch_height: EpochHeight,
    pub last_locked_account_balance: Balance,
    pub last_account_balance: Balance,
    pub total_staked_shares: Balance,
    pub total_staked_balance: Balance,
    pub accounts: Map<AccountId, Account>,
}

impl Default for StakingContract {
    fn default() -> Self {
        env::panic(b"Staking contract should be initialized before usage")
    }
}

#[near_bindgen]
impl StakingContract {
    /// Call to initialize the contract.
    /// Specify which account can change the staking key and the initial staking key with ED25519 curve.
    #[init]
    pub fn new(owner_id: AccountId, stake_public_key: Base58PublicKey) -> Self {
        assert!(!env::state_exists(), "Already initialized");
        Self {
            owner_id,
            stake_public_key: stake_public_key
                .try_into()
                .expect("invalid staking public key"),
            last_epoch_height: env::epoch_height(),
            last_locked_account_balance: 0,
            last_account_balance: env::account_balance(),
            total_staked_balance: 0,
            total_staked_shares: 0,
            accounts: Map::new(b"u".to_vec()),
        }
    }

    /// Call to update state after epoch switched.
    pub fn ping(&mut self) {
        // Checking if we need there are rewards to distribute.
        let epoch_height = env::epoch_height();
        if self.last_epoch_height == epoch_height {
            return;
        }
        self.last_epoch_height = epoch_height;

        let total_balance =
            env::account_locked_balance() + env::account_balance() - env::attached_deposit();
        let last_total_balance = self.last_account_balance + self.last_locked_account_balance;

        let total_reward = total_balance.saturating_sub(last_total_balance);
        // The total stake is increased by both staked and unstaked rewards.
        self.total_staked_balance += total_reward;

        self.last_locked_account_balance = env::account_locked_balance();
        self.last_account_balance = env::account_balance();
    }

    /// Call to deposit money.
    #[payable]
    pub fn deposit(&mut self) {
        self.ping();

        let account_id = env::predecessor_account_id();
        let mut account = self.get_account(&account_id);
        account.unstaked += env::attached_deposit();
        self.save_account(&account_id, account);
        self.last_account_balance = env::account_balance();

        // Potentially restake in case in the locked amount is smaller than the desired staked
        // balance due to unstake in the past.
        if self.total_staked_balance > self.last_locked_account_balance {
            self.restake();
        }
    }

    /// Withdraws the non staked balance for given account.
    pub fn withdraw(&mut self, amount: U128) {
        self.ping();

        let amount = amount.into();
        assert!(amount > 0, "Withdrawal amount should be positive");

        let account_id = env::predecessor_account_id();
        let mut account = self.get_account(&account_id);
        assert!(
            account.unstaked >= amount,
            "Not enough unstaked balance to withdraw"
        );
        assert!(
            account.unstaked_available_epoch_height <= env::epoch_height(),
            "The unstaked balance is not yet available due to unstaking delay"
        );
        account.unstaked -= amount;
        self.save_account(&account_id, account);
        Promise::new(account_id).transfer(amount);
        self.last_account_balance = env::account_balance();

        // Potentially restake in case in the locked amount is smaller than the desired staked
        // balance due to unstake in the past.
        if self.total_staked_balance > self.last_locked_account_balance {
            self.restake();
        }
    }

    /// Stakes previously deposited money by given account on this account.
    pub fn stake(&mut self, amount: U128) {
        self.ping();

        let amount: Balance = amount.into();
        assert!(amount > 0, "Staking amount should be positive");

        let account_id = env::predecessor_account_id();
        let mut account = self.get_account(&account_id);

        // NOTE: Number of shares the account gets is rounded up, but the gas rebate within
        // contract will compensate for the rounding errors.
        let num_shares = self.num_shares_from_amount_rounded_up(amount);

        assert!(
            account.unstaked >= amount,
            "Not enough unstaked balance to stake"
        );
        account.unstaked -= amount;
        account.staked_shares += num_shares;
        self.save_account(&account_id, account);

        self.total_staked_balance += amount;
        self.total_staked_shares += num_shares;

        self.restake();
    }

    /// Request withdrawal for epoch + 2 by sending unstaking transaction for
    /// `current locked - (given account deposit + rewards)`
    pub fn unstake(&mut self, amount: U128) {
        self.ping();

        let amount: Balance = amount.into();
        assert!(amount > 0, "Unstaking amount should be positive");

        let account_id = env::predecessor_account_id();
        let mut account = self.get_account(&account_id);

        assert!(
            self.total_staked_balance > 0,
            "The contract doesn't have staked balance"
        );
        // NOTE: The number of shares the account will pay is rounded up, to avoid giving extra
        // amount.
        let num_shares = self.num_shares_from_amount_rounded_up(amount);
        assert!(
            account.staked_shares >= num_shares,
            "Not enough staked balance to unstake"
        );
        account.staked_shares -= num_shares;
        account.unstaked += amount;
        account.unstaked_available_epoch_height = env::epoch_height() + EPOCHS_TOWARDS_REWARD;
        self.save_account(&account_id, account);

        self.total_staked_balance -= amount;
        self.total_staked_shares -= num_shares;

        self.restake();
    }

    /// Returns the number of shares corresponding to the given amount rounded up.
    ///
    /// price = total_staked / total_shares
    /// Price is fixed
    /// (total_staked + amount) / (total_shares + num_shares) = total_staked / total_shares
    /// (total_staked + amount) * total_shares = total_staked * (total_shares + num_shares)
    /// amount * total_shares = total_staked * num_shares
    /// num_shares = amount * total_shares / total_staked
    /// Rounding up division of `a / b` is done using `(a + b - 1) / b`.
    fn num_shares_from_amount_rounded_up(&self, amount: Balance) -> Balance {
        if self.total_staked_balance == 0 {
            return amount;
        }
        ((U256::from(self.total_staked_shares) * U256::from(amount)
            + U256::from(self.total_staked_balance - 1))
            / U256::from(self.total_staked_balance))
        .as_u128()
    }

    /// Restakes the current `total_staked_balance` again.
    fn restake(&mut self) {
        Promise::new(env::current_account_id())
            .function_call(b"ping".to_vec(), b"{}".to_vec(), 0, PING_GAS)
            .stake(self.total_staked_balance, self.stake_public_key.clone())
            .function_call(
                b"internal_after_stake".to_vec(),
                b"{}".to_vec(),
                0,
                INTERNAL_AFTER_STAKE_GAS,
            );
    }

    /// Private method to be called after stake action.
    pub fn internal_after_stake(&mut self) {
        assert_eq!(env::current_account_id(), env::predecessor_account_id());
        self.last_account_balance = env::account_balance();
        self.last_locked_account_balance = env::account_locked_balance();
    }

    /// Returns given account's unstaked balance.
    pub fn get_account_unstaked_balance(&self, account_id: AccountId) -> U128 {
        self.get_account(&account_id).unstaked.into()
    }

    /// Returns given account's staked balance.
    pub fn get_account_staked_balance(&self, account_id: AccountId) -> U128 {
        if self.total_staked_shares > 0 {
            // Rounding down
            (U256::from(self.total_staked_balance)
                * U256::from(self.get_account(&account_id).staked_shares)
                / U256::from(self.total_staked_shares))
            .as_u128()
            .into()
        } else {
            0.into()
        }
    }

    pub fn get_account_total_balance(&self, account_id: AccountId) -> U128 {
        (self.get_account(&account_id).unstaked + self.get_account_staked_balance(account_id).0)
            .into()
    }

    pub fn is_account_unstaked_balance_available(&self, account_id: AccountId) -> bool {
        self.get_account(&account_id)
            .unstaked_available_epoch_height
            <= env::epoch_height()
    }

    fn get_account(&self, account_id: &AccountId) -> Account {
        self.accounts.get(account_id).unwrap_or_default()
    }

    fn save_account(&mut self, account_id: &AccountId, account: Account) {
        if account.unstaked > 0 || account.staked_shares > 0 {
            self.accounts.insert(&account_id, &account);
        } else {
            self.accounts.remove(&account_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use near_sdk::{testing_env, MockedBlockchain};

    use crate::test_utils::*;

    use super::*;
    use std::convert::TryFrom;

    struct Emulator {
        pub contract: StakingContract,
        pub epoch_height: EpochHeight,
        pub amount: Balance,
        pub locked_amount: Balance,
    }

    impl Emulator {
        pub fn new(owner: String, stake_public_key: String) -> Self {
            testing_env!(VMContextBuilder::new()
                .current_account_id(owner.clone())
                .finish());
            Emulator {
                contract: StakingContract::new(
                    owner,
                    Base58PublicKey::try_from(stake_public_key).unwrap(),
                ),
                epoch_height: 0,
                amount: 0,
                locked_amount: 0,
            }
        }

        pub fn update_context(&mut self, predecessor_account_id: String, deposit: Balance) {
            testing_env!(VMContextBuilder::new()
                .current_account_id(staking())
                .predecessor_account_id(predecessor_account_id.clone())
                .signer_account_id(predecessor_account_id)
                .attached_deposit(deposit)
                .account_balance(self.amount)
                .account_locked_balance(self.locked_amount)
                .epoch_height(self.epoch_height)
                .finish());
            println!(
                "Epoch: {}, Deposit: {}, amount: {}, locked_amount: {}",
                self.epoch_height, deposit, self.amount, self.locked_amount
            );
        }

        pub fn simulate_stake_call(&mut self) {
            self.update_context(staking(), 0);
            let total_stake = self.contract.total_staked_balance;
            // First function call action
            self.contract.ping();
            // Stake action
            self.amount = self.amount + self.locked_amount - total_stake;
            self.locked_amount = total_stake;
            // Second function call action
            self.update_context(staking(), 0);
            self.contract.internal_after_stake();
        }

        pub fn skip_epochs(&mut self, num: EpochHeight) {
            self.epoch_height += num;
            self.locked_amount = (self.locked_amount * (100 + u128::from(num))) / 100;
        }
    }

    #[test]
    fn test_deposit_withdraw() {
        let mut emulator = Emulator::new(
            "owner".to_string(),
            "KuTCtARNzxZQ3YvXDeLjx83FDqxv2SdQTSbiq876zR7".to_string(),
        );
        let deposit_amount = 1_000_000;
        emulator.update_context(bob(), deposit_amount);
        emulator.contract.deposit();
        emulator.amount += deposit_amount;
        emulator.update_context(bob(), 0);
        assert_eq!(
            emulator.contract.get_account_unstaked_balance(bob()).0,
            deposit_amount
        );
        emulator.contract.withdraw(deposit_amount.into());
        assert_eq!(
            emulator.contract.get_account_unstaked_balance(bob()).0,
            0u128
        );
    }

    #[test]
    fn test_stake_unstake() {
        let mut emulator = Emulator::new(
            "owner".to_string(),
            "KuTCtARNzxZQ3YvXDeLjx83FDqxv2SdQTSbiq876zR7".to_string(),
        );
        let deposit_amount = 1_000_000;
        emulator.update_context(bob(), deposit_amount);
        emulator.contract.deposit();
        emulator.amount += deposit_amount;
        emulator.update_context(bob(), 0);
        emulator.contract.stake(deposit_amount.into());
        emulator.simulate_stake_call();
        assert_eq!(
            emulator.contract.get_account_staked_balance(bob()).0,
            deposit_amount
        );
        // 10 epochs later, unstake half of the money.
        emulator.skip_epochs(10);
        // Overriding rewards
        emulator.locked_amount = deposit_amount + 10;
        emulator.update_context(bob(), 0);
        emulator.contract.ping();
        assert_eq!(
            emulator.contract.get_account_staked_balance(bob()).0,
            deposit_amount + 10
        );
        emulator.contract.unstake((deposit_amount / 2).into());
        emulator.simulate_stake_call();
        assert_eq!(
            emulator.contract.get_account_staked_balance(bob()).0,
            deposit_amount / 2 + 10
        );
        assert_eq!(
            emulator.contract.get_account_unstaked_balance(bob()).0,
            deposit_amount / 2
        );
        assert!(!emulator
            .contract
            .is_account_unstaked_balance_available(bob()),);
        emulator.skip_epochs(3);
        emulator.update_context(bob(), 0);
        assert!(emulator
            .contract
            .is_account_unstaked_balance_available(bob()),);
    }

    /// Test that two can delegate and then undelegate their funds and rewards at different time.
    #[test]
    fn test_two_delegates() {
        let mut emulator = Emulator::new(
            "owner".to_string(),
            "KuTCtARNzxZQ3YvXDeLjx83FDqxv2SdQTSbiq876zR7".to_string(),
        );
        emulator.update_context(alice(), 1_000_000);
        emulator.contract.deposit();
        emulator.amount += 1_000_000;
        emulator.update_context(alice(), 0);
        emulator.contract.stake(1_000_000.into());
        emulator.simulate_stake_call();
        emulator.skip_epochs(3);
        emulator.update_context(bob(), 1_000_000);

        emulator.contract.deposit();
        emulator.amount += 1_000_000;
        emulator.update_context(bob(), 0);
        emulator.contract.stake(1_000_000.into());
        emulator.simulate_stake_call();
        assert_eq!(
            emulator.contract.get_account_staked_balance(bob()).0,
            1_000_000
        );
        emulator.skip_epochs(3);
        emulator.update_context(alice(), 0);
        emulator.contract.ping();
        assert_eq!(
            emulator.contract.get_account_staked_balance(alice()).0,
            1060899
        );
        assert_eq!(
            emulator.contract.get_account_staked_balance(bob()).0,
            1030000
        );
    }
}
