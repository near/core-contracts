use borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::collections::Map as NearMap;
use near_sdk::{env, near_bindgen, AccountId, Balance, EpochHeight, Promise, PublicKey};

use utils::U128;

#[cfg(test)]
mod test_utils;

mod utils;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[derive(BorshDeserialize, BorshSerialize, Debug)]
pub struct User {
    pub account_id: AccountId,
    pub amount: Balance,
    pub stake: Balance,
    pub stake_epoch_height: EpochHeight,
}

impl User {
    pub fn new(account_id: &AccountId, amount: Balance) -> Self {
        Self {
            account_id: account_id.clone(),
            amount,
            stake: 0,
            stake_epoch_height: 0,
        }
    }

    pub fn stake(&mut self, amount: Balance) {
        self.amount -= amount;
        self.stake += amount;
        self.stake_epoch_height = env::epoch_height();
    }

    pub fn unstake(&mut self, amount: Balance) {
        self.stake -= amount;
        self.amount += amount;
        self.stake_epoch_height = env::epoch_height();
    }

    /// Checks if given user has enough non staked/locked balance and withdraws it.
    pub fn withdraw(&mut self, amount: Balance) {
        // TODO
        self.amount -= amount;
    }
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct StakingContract {
    pub owner: AccountId,
    pub stake_public_key: PublicKey,
    pub prev_staked_amount: Balance,
    pub staked_amount: Balance,
    pub users: NearMap<AccountId, User>,
}

impl Default for StakingContract {
    fn default() -> Self {
        env::panic(b"Staking token should be initialized before usage")
    }
}

#[near_bindgen]
impl StakingContract {
    /// Call to initialize the contract.
    /// Specify which account can change the staking key and the initial staking key with ED25519 curve.
    #[init]
    pub fn new(owner: AccountId, stake_public_key: String) -> Self {
        assert!(
            env::state_read::<StakingContract>().is_none(),
            "Already initialized"
        );
        let mut pk = vec![0];
        pk.extend(bs58::decode(stake_public_key).into_vec().unwrap());
        Self {
            owner,
            stake_public_key: pk,
            prev_staked_amount: 0,
            staked_amount: 0,
            users: NearMap::default(),
        }
    }

    /// Call to update state after epoch switched.
    pub fn ping(&mut self) {
        // Epoch passed, we received rewards and need to redistribute it to users.
        assert!(
            env::account_locked_balance() >= self.staked_amount,
            "The logic of the contract was broken"
        );
        let reward = env::account_locked_balance() - self.staked_amount;
        //        println!("Reward: {:?}", reward);
        if reward > 0 {
            // (reward / staked_amount) * amount
            let mut new_users = vec![];
            for (account_id, mut user) in self.users.iter() {
                if user.stake_epoch_height < env::epoch_height() {
                    user.stake = user.stake + (user.stake * reward) / self.staked_amount;
                    new_users.push((account_id, user));
                }
            }
            //            println!("New users: {:?}", new_users);
            for (account_id, user) in new_users {
                self.users.insert(&account_id, &user);
            }
        }
    }

    /// Call to deposit money.
    pub fn deposit(&mut self) {
        self.ping();
        let account_id = env::predecessor_account_id();
        let user = if let Some(mut user) = self.users.get(&account_id) {
            user.amount += env::attached_deposit();
            user
        } else {
            User::new(&account_id, env::attached_deposit())
        };
        self.users.insert(&account_id, &user);
    }

    /// Stakes previously deposited money by given user on this account.
    pub fn stake(&mut self, amount: U128) {
        let amount = amount.into();
        self.ping();
        let account_id = env::predecessor_account_id();
        let mut user = self.users.get(&account_id).expect("User is missing");
        user.stake(amount);
        self.users.insert(&account_id, &user);
        ////        println!("{:?} stake: {}, staked: {}, locked: {}", user, amount, self.staked_amount, env::account_locked_balance());
        self.staked_amount += amount;
        Promise::new(env::current_account_id())
            .stake(self.staked_amount, self.stake_public_key.clone());
    }

    /// Withdraws the non staked balance for given user.
    pub fn withdraw(&mut self, amount: U128) {
        let amount = amount.into();
        self.ping();
        let account_id = env::predecessor_account_id();
        let mut user = self.users.get(&account_id).expect("User is missing");
        user.withdraw(amount);
        self.users.insert(&account_id, &user);
        Promise::new(account_id).transfer(amount);
    }

    /// Request withdrawal for epoch + 2 by sending unstaking transaction for
    /// `current locked - (given user deposit + rewards)`
    pub fn unstake(&mut self, amount: U128) {
        let amount = amount.into();
        self.ping();
        let account_id = env::predecessor_account_id();
        let mut user = self.users.get(&account_id).expect("User is missing");
        assert!(self.staked_amount >= amount);
        user.unstake(amount);
        self.users.insert(&account_id, &user);
        self.staked_amount -= amount;
        //        println!("{:?} unstake {}, staked: {}, locked: {}", user, amount, self.staked_amount, env::account_locked_balance());
        Promise::new(env::current_account_id())
            .stake(self.staked_amount, self.stake_public_key.clone());
    }

    /// Returns given user's liquid balance.
    pub fn get_user_balance(&mut self, account_id: AccountId) -> U128 {
        let user = self.users.get(&account_id).expect("User is missing");
        user.amount.into()
    }

    /// Returns given user's staked balance.
    pub fn get_user_stake(&mut self, account_id: AccountId) -> U128 {
        let user = self.users.get(&account_id).expect("User is missing");
        user.stake.into()
    }
}

#[cfg(test)]
mod tests {
    use near_sdk::{testing_env, MockedBlockchain};

    use crate::test_utils::*;

    use super::*;

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
                contract: StakingContract::new(owner, stake_public_key),
                epoch_height: 0,
                amount: 0,
                locked_amount: 0,
            }
        }

        pub fn call_function(
            &mut self,
            caller: String,
            deposit: Balance,
            f: fn(&mut StakingContract),
        ) {
            testing_env!(VMContextBuilder::new()
                .current_account_id(caller)
                .attached_deposit(deposit)
                .account_balance(self.amount)
                .account_locked_balance(self.locked_amount)
                .finish());
            println!("Deposit: {}, amount: {}, locked_amount: {}", deposit, self.amount, self.locked_amount);
            f(&mut self.contract);
        }

        pub fn skip_epochs(&mut self, num: EpochHeight) {
            self.epoch_height += num;
            self.locked_amount = (self.locked_amount * 101 * u128::from(num)) / 100;
        }
    }

    #[test]
    fn test_deposit_withdraw() {
        testing_env!(VMContextBuilder::new()
            .current_account_id("owner".to_string())
            .finish());
        let mut contract = StakingContract::new("owner".to_string(), "7LmTyhMqQ3nxAY6t78QoH4Dc1pRUq1S9mxtyXLdYKjVjWH7EWYdVA3YzJk5o1sMB5JrvKrzTwCAZ2HgiYhPgm6k".to_string());
        let deposit_amount = 1_000_000;
        testing_env!(VMContextBuilder::new()
            .current_account_id(staking())
            .predecessor_account_id(bob())
            .attached_deposit(deposit_amount)
            .finish());
        contract.deposit();
        testing_env!(VMContextBuilder::new()
            .current_account_id(staking())
            .predecessor_account_id(bob())
            .account_balance(deposit_amount)
            .finish());
        assert_eq!(contract.get_user_balance(bob()), deposit_amount);
        contract.withdraw(deposit_amount.into());
        assert_eq!(contract.get_user_balance(bob()), 0u128);
    }

    #[test]
    fn test_stake_unstake() {
        testing_env!(VMContextBuilder::new()
            .current_account_id("owner".to_string())
            .finish());
        let mut contract = StakingContract::new("owner".to_string(), "7LmTyhMqQ3nxAY6t78QoH4Dc1pRUq1S9mxtyXLdYKjVjWH7EWYdVA3YzJk5o1sMB5JrvKrzTwCAZ2HgiYhPgm6k".to_string());
        let deposit_amount = 1_000_000;
        testing_env!(VMContextBuilder::new()
            .current_account_id(staking())
            .predecessor_account_id(bob())
            .attached_deposit(deposit_amount)
            .finish());
        contract.deposit();
        testing_env!(VMContextBuilder::new()
            .current_account_id(staking())
            .predecessor_account_id(bob())
            .account_balance(deposit_amount)
            .finish());
        contract.stake(deposit_amount.into());
        // 10 epochs later, unstake half of the money.
        testing_env!(VMContextBuilder::new()
            .current_account_id(staking())
            .predecessor_account_id(bob())
            .epoch_height(10)
            .account_locked_balance(deposit_amount + 10)
            .finish());
        assert_eq!(contract.get_user_stake(bob()), deposit_amount);
        contract.unstake((deposit_amount / 2).into());
        assert_eq!(contract.get_user_stake(bob()), deposit_amount / 2 + 10);
        assert_eq!(contract.get_user_balance(bob()), deposit_amount / 2);
    }

    /// Test that two can deleegate and then undelegate their funds and rewards at different time.
    #[test]
    #[ignore]
    fn test_two_delegates() {
        let mut emulator = Emulator::new("owner".to_string(), "7LmTyhMqQ3nxAY6t78QoH4Dc1pRUq1S9mxtyXLdYKjVjWH7EWYdVA3YzJk5o1sMB5JrvKrzTwCAZ2HgiYhPgm6k".to_string());
        emulator.call_function(alice(), 1_000_000, |contract| contract.deposit());
        emulator.call_function(alice(), 0, |contract| contract.stake(1_000_000.into()));
        emulator.skip_epochs(2);
        emulator.call_function(bob(), 1_000_000, |contract| contract.deposit());
        emulator.call_function(bob(), 0, |contract| contract.stake(1_000_000.into()));
        emulator.skip_epochs(2);
    }
}
