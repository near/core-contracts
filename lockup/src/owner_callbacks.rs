use crate::*;
use near_sdk::{near_bindgen, PromiseOrValue, assert_self, is_promise_success};

#[near_bindgen]
impl LockupContract {
    /// Called after a given `staking_pool_account_id` was checked in the whitelist.
    pub fn on_whitelist_is_whitelisted(
        &mut self,
        #[callback] is_whitelisted: bool,
        staking_pool_account_id: AccountId,
    ) -> bool {
        assert_self();
        assert!(
            is_whitelisted,
            "The given staking pool account ID is not whitelisted"
        );
        self.assert_staking_pool_is_not_selected();
        self.assert_no_termination();
        self.staking_information = Some(StakingInformation {
            staking_pool_account_id,
            status: TransactionStatus::Idle,
            deposit_amount: 0.into(),
        });
        true
    }

    /// Called after a deposit amount was transferred out of this account to the staking pool.
    /// This method needs to update staking pool status.
    pub fn on_staking_pool_deposit(&mut self, amount: WrappedBalance) -> bool {
        assert_self();

        let deposit_succeeded = is_promise_success();
        self.set_staking_pool_status(TransactionStatus::Idle);

        if deposit_succeeded {
            self.staking_information.as_mut().unwrap().deposit_amount.0 += amount.0;
            env::log(
                format!(
                    "The deposit of {} to @{} succeeded",
                    amount.0,
                    self.staking_information
                        .as_ref()
                        .unwrap()
                        .staking_pool_account_id
                )
                .as_bytes(),
            );
        } else {
            env::log(
                format!(
                    "The deposit of {} to @{} has failed",
                    amount.0,
                    self.staking_information
                        .as_ref()
                        .unwrap()
                        .staking_pool_account_id
                )
                .as_bytes(),
            );
        }
        deposit_succeeded
    }

    /// Called after a deposit amount was transferred out of this account to the staking pool and it
    /// was staked on the staking pool.
    /// This method needs to update staking pool status.
    pub fn on_staking_pool_deposit_and_stake(&mut self, amount: WrappedBalance) -> bool {
        assert_self();

        let deposit_and_stake_succeeded = is_promise_success();
        self.set_staking_pool_status(TransactionStatus::Idle);

        if deposit_and_stake_succeeded {
            self.staking_information.as_mut().unwrap().deposit_amount.0 += amount.0;
            env::log(
                format!(
                    "The deposit and stake of {} to @{} succeeded",
                    amount.0,
                    self.staking_information
                        .as_ref()
                        .unwrap()
                        .staking_pool_account_id
                )
                .as_bytes(),
            );
        } else {
            env::log(
                format!(
                    "The deposit and stake of {} to @{} has failed",
                    amount.0,
                    self.staking_information
                        .as_ref()
                        .unwrap()
                        .staking_pool_account_id
                )
                .as_bytes(),
            );
        }
        deposit_and_stake_succeeded
    }

    /// Called after the given amount was requested to transfer out from the staking pool to this
    /// account.
    /// This method needs to update staking pool status.
    pub fn on_staking_pool_withdraw(&mut self, amount: WrappedBalance) -> bool {
        assert_self();

        let withdraw_succeeded = is_promise_success();
        self.set_staking_pool_status(TransactionStatus::Idle);

        if withdraw_succeeded {
            {
                let staking_information = self.staking_information.as_mut().unwrap();
                // Due to staking rewards the deposit amount can become negative.
                staking_information.deposit_amount.0 = staking_information
                    .deposit_amount
                    .0
                    .saturating_sub(amount.0);
            }
            env::log(
                format!(
                    "The withdrawal of {} from @{} succeeded",
                    amount.0,
                    self.staking_information
                        .as_ref()
                        .unwrap()
                        .staking_pool_account_id
                )
                .as_bytes(),
            );
        } else {
            env::log(
                format!(
                    "The withdrawal of {} from @{} failed",
                    amount.0,
                    self.staking_information
                        .as_ref()
                        .unwrap()
                        .staking_pool_account_id
                )
                .as_bytes(),
            );
        }
        withdraw_succeeded
    }

    /// Called after the extra amount stake was staked in the staking pool contract.
    /// This method needs to update staking pool status.
    pub fn on_staking_pool_stake(&mut self, amount: WrappedBalance) -> bool {
        assert_self();

        let stake_succeeded = is_promise_success();
        self.set_staking_pool_status(TransactionStatus::Idle);

        if stake_succeeded {
            env::log(
                format!(
                    "Staking of {} at @{} succeeded",
                    amount.0,
                    self.staking_information
                        .as_ref()
                        .unwrap()
                        .staking_pool_account_id
                )
                .as_bytes(),
            );
        } else {
            env::log(
                format!(
                    "Staking {} at @{} has failed",
                    amount.0,
                    self.staking_information
                        .as_ref()
                        .unwrap()
                        .staking_pool_account_id
                )
                .as_bytes(),
            );
        }
        stake_succeeded
    }

    /// Called after the given amount was unstaked at the staking pool contract.
    /// This method needs to update staking pool status.
    pub fn on_staking_pool_unstake(&mut self, amount: WrappedBalance) -> bool {
        assert_self();

        let unstake_succeeded = is_promise_success();
        self.set_staking_pool_status(TransactionStatus::Idle);

        if unstake_succeeded {
            env::log(
                format!(
                    "Unstaking of {} at @{} succeeded",
                    amount.0,
                    self.staking_information
                        .as_ref()
                        .unwrap()
                        .staking_pool_account_id
                )
                .as_bytes(),
            );
        } else {
            env::log(
                format!(
                    "Unstaking {} at @{} has failed",
                    amount.0,
                    self.staking_information
                        .as_ref()
                        .unwrap()
                        .staking_pool_account_id
                )
                .as_bytes(),
            );
        }
        unstake_succeeded
    }

    /// Called after all tokens were unstaked at the staking pool contract
    /// This method needs to update staking pool status.
    pub fn on_staking_pool_unstake_all(&mut self) -> bool {
        assert_self();

        let unstake_all_succeeded = is_promise_success();
        self.set_staking_pool_status(TransactionStatus::Idle);

        if unstake_all_succeeded {
            env::log(
                format!(
                    "Unstaking all at @{} succeeded",
                    self.staking_information
                        .as_ref()
                        .unwrap()
                        .staking_pool_account_id
                )
                .as_bytes(),
            );
        } else {
            env::log(
                format!(
                    "Unstaking all at @{} has failed",
                    self.staking_information
                        .as_ref()
                        .unwrap()
                        .staking_pool_account_id
                )
                .as_bytes(),
            );
        }
        unstake_all_succeeded
    }

    /// Called after the transfer voting contract was checked for the vote result.
    pub fn on_get_result_from_transfer_poll(
        &mut self,
        #[callback] poll_result: PollResult,
    ) -> bool {
        assert_self();
        self.assert_transfers_disabled();

        if let Some(transfers_timestamp) = poll_result {
            env::log(
                format!(
                    "Transfers were successfully enabled at {}",
                    transfers_timestamp.0
                )
                .as_bytes(),
            );
            self.lockup_information.transfers_information =
                TransfersInformation::TransfersEnabled {
                    transfers_timestamp,
                };
            true
        } else {
            env::log(b"The transfers are not enabled yet");
            false
        }
    }

    /// Called after the request to get the current total balance from the staking pool.
    pub fn on_get_account_total_balance(&mut self, #[callback] total_balance: WrappedBalance) {
        assert_self();
        self.set_staking_pool_status(TransactionStatus::Idle);

        env::log(
            format!(
                "The current total balance on the staking pool is {}",
                total_balance.0
            )
            .as_bytes(),
        );

        self.staking_information.as_mut().unwrap().deposit_amount = total_balance;
    }

    /// Called after the request to get the current unstaked balance to withdraw everything by th
    /// owner.
    pub fn on_get_account_unstaked_balance_to_withdraw_by_owner(
        &mut self,
        #[callback] unstaked_balance: WrappedBalance,
    ) -> PromiseOrValue<bool> {
        assert_self();
        if unstaked_balance.0 > 0 {
            // Need to withdraw
            env::log(
                format!(
                    "Withdrawing {} from the staking pool @{}",
                    unstaked_balance.0,
                    self.staking_information
                        .as_ref()
                        .unwrap()
                        .staking_pool_account_id
                )
                .as_bytes(),
            );

            ext_staking_pool::withdraw(
                unstaked_balance,
                &self
                    .staking_information
                    .as_ref()
                    .unwrap()
                    .staking_pool_account_id,
                NO_DEPOSIT,
                gas::staking_pool::WITHDRAW,
            )
            .then(ext_self_owner::on_staking_pool_withdraw(
                unstaked_balance,
                &env::current_account_id(),
                NO_DEPOSIT,
                gas::owner_callbacks::ON_STAKING_POOL_WITHDRAW,
            ))
            .into()
        } else {
            env::log(b"No unstaked balance on the staking pool to withdraw");
            self.set_staking_pool_status(TransactionStatus::Idle);
            PromiseOrValue::Value(true)
        }
    }
}
