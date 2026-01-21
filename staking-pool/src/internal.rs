use crate::*;

impl StakingContract {
    /********************/
    /* Internal methods */
    /********************/

    /// Restakes the current total staked balance.
    pub(crate) fn internal_restake(&mut self) {
        if self.paused {
            return;
        }

        // Optimized promise call: avoid excessive cloning if possible
        Promise::new(env::current_account_id())
            .stake(self.total_staked_balance, self.stake_public_key.clone())
            .then(ext_self::on_stake_action(
                &env::current_account_id(),
                NO_DEPOSIT,
                ON_STAKE_ACTION_GAS,
            ));
    }

    /// Handles deposits and updates the global tracking balance.
    pub(crate) fn internal_deposit(&mut self) -> u128 {
        let account_id = env::predecessor_account_id();
        let mut account = self.internal_get_account(&account_id);
        let amount = env::attached_deposit();
        
        account.unstaked += amount;
        self.internal_save_account(&account_id, &account);
        
        // Critical: Update total tracking balance to prevent reward dilution
        self.last_total_balance += amount;

        env::log_str(&format!("@{} deposited {}. Unstaked: {}", account_id, amount, account.unstaked));
        amount
    }

    /// Optimized withdrawal with strict CEI (Checks-Effects-Interactions) compliance.
    pub(crate) fn internal_withdraw(&mut self, amount: Balance) {
        assert!(amount > 0, "ERR_WITHDRAW_POSITIVE");

        let account_id = env::predecessor_account_id();
        let mut account = self.internal_get_account(&account_id);
        
        assert!(account.unstaked >= amount, "ERR_NOT_ENOUGH_UNSTAKED");
        assert!(account.unstaked_available_epoch_height <= env::epoch_height(), "ERR_WITHDRAW_LOCKED");

        // Effect: update state before interaction
        account.unstaked -= amount;
        self.last_total_balance -= amount;
        self.internal_save_account(&account_id, &account);

        env::log_str(&format!("@{} withdrawing {}. Remaining: {}", account_id, amount, account.unstaked));

        // Interaction: External transfer
        Promise::new(account_id).transfer(amount);
    }

    /// Precision-optimized staking logic.
    pub(crate) fn internal_stake(&mut self, amount: Balance) {
        assert!(amount > 0, "ERR_STAKE_POSITIVE");

        let account_id = env::predecessor_account_id();
        let mut account = self.internal_get_account(&account_id);

        let num_shares = self.num_shares_from_staked_amount_rounded_down(amount);
        assert!(num_shares > 0, "ERR_STAKE_TOO_SMALL");

        let charge_amount = self.staked_amount_from_num_shares_rounded_down(num_shares);
        assert!(account.unstaked >= charge_amount, "ERR_NOT_ENOUGH_UNSTAKED");

        account.unstaked -= charge_amount;
        account.stake_shares += num_shares;
        self.internal_save_account(&account_id, &account);

        let stake_amount = self.staked_amount_from_num_shares_rounded_up(num_shares);
        self.total_staked_balance += stake_amount;
        self.total_stake_shares += num_shares;

        env::log_str(&format!("@{} staked {}. Shares: {}", account_id, charge_amount, num_shares));
    }

    /// Handles unstaking with epoch locking for security.
    pub(crate) fn inner_unstake(&mut self, amount: u128) {
        assert!(amount > 0, "ERR_UNSTAKE_POSITIVE");

        let account_id = env::predecessor_account_id();
        let mut account = self.internal_get_account(&account_id);

        assert!(self.total_staked_balance > 0, "ERR_CONTRACT_EMPTY");

        let num_shares = self.num_shares_from_staked_amount_rounded_up(amount);
        assert!(account.stake_shares >= num_shares, "ERR_NOT_ENOUGH_SHARES");

        let receive_amount = self.staked_amount_from_num_shares_rounded_up(num_shares);
        let unstake_amount = self.staked_amount_from_num_shares_rounded_down(num_shares);

        account.stake_shares -= num_shares;
        account.unstaked += receive_amount;
        account.unstaked_available_epoch_height = env::epoch_height() + NUM_EPOCHS_TO_UNLOCK;
        self.internal_save_account(&account_id, &account);

        self.total_staked_balance -= unstake_amount;
        self.total_stake_shares -= num_shares;

        env::log_str(&format!("@{} unstaked {}. Shares spent: {}", account_id, receive_amount, num_shares));
    }

    /// Distributes rewards and takes owner commission.
    pub(crate) fn internal_ping(&mut self) -> bool {
        let epoch_height = env::epoch_height();
        if self.last_epoch_height == epoch_height {
            return false;
        }
        self.last_epoch_height = epoch_height;

        let total_balance = env::account_locked_balance() + env::account_balance() - env::attached_deposit();

        if total_balance > self.last_total_balance {
            let total_reward = total_balance - self.last_total_balance;
            let owners_fee = self.reward_fee_fraction.multiply(total_reward);
            let remaining_reward = total_reward - owners_fee;

            // Update total balance with remaining rewards first
            self.total_staked_balance += remaining_reward;

            let num_shares = self.num_shares_from_staked_amount_rounded_down(owners_fee);
            if num_shares > 0 {
                let mut owner_account = self.internal_get_account(&self.owner_id);
                owner_account.stake_shares += num_shares;
                self.total_stake_shares += num_shares;
                self.internal_save_account(&self.owner_id.clone(), &owner_account);
            }
            
            // Consistently update global staked balance
            self.total_staked_balance += owners_fee;

            env::log_str(&format!("Epoch {}: Rewards distributed. Total Staked: {}", epoch_height, self.total_staked_balance));
        }

        self.last_total_balance = total_balance;
        true
    }

    /*******************/
    /* Math Operations */
    /*******************/

    pub(crate) fn num_shares_from_staked_amount_rounded_down(&self, amount: Balance) -> NumStakeShares {
        assert!(self.total_staked_balance > 0, "ERR_ZERO_STAKED_BALANCE");
        (U256::from(self.total_stake_shares) * U256::from(amount) / U256::from(self.total_staked_balance)).as_u128()
    }

    pub(crate) fn num_shares_from_staked_amount_rounded_up(&self, amount: Balance) -> NumStakeShares {
        assert!(self.total_staked_balance > 0, "ERR_ZERO_STAKED_BALANCE");
        ((U256::from(self.total_stake_shares) * U256::from(amount) + U256::from(self.total_staked_balance - 1)) / U256::from(self.total_staked_balance)).as_u128()
    }

    pub(crate) fn staked_amount_from_num_shares_rounded_down(&self, num_shares: NumStakeShares) -> Balance {
        assert!(self.total_stake_shares > 0, "ERR_ZERO_SHARES");
        (U256::from(self.total_staked_balance) * U256::from(num_shares) / U256::from(self.total_stake_shares)).as_u128()
    }

    pub(crate) fn staked_amount_from_num_shares_rounded_up(&self, num_shares: NumStakeShares) -> Balance {
        assert!(self.total_stake_shares > 0, "ERR_ZERO_SHARES");
        ((U256::from(self.total_staked_balance) * U256::from(num_shares) + U256::from(self.total_stake_shares - 1)) / U256::from(self.total_stake_shares)).as_u128()
    }

    pub(crate) fn internal_get_account(&self, account_id: &AccountId) -> Account {
        self.accounts.get(account_id).unwrap_or_default()
    }

    pub(crate) fn internal_save_account(&mut self, account_id: &AccountId, account: &Account) {
        if account.unstaked > 0 || account.stake_shares > 0 {
            self.accounts.insert(account_id, account);
        } else {
            self.accounts.remove(account_id);
        }
    }
}
