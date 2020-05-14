use crate::*;
use near_sdk::near_bindgen;

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
        self.staking_information = Some(StakingInformation {
            staking_pool_account_id,
            status: TransactionStatus::Idle,
            deposit_amount: 0.into(),
        });
        true
    }

    /// Called after there was a request to unselect current staking pool.
    pub fn on_staking_pool_get_total_user_balance(
        &mut self,
        #[callback] total_balance: WrappedBalance,
    ) -> bool {
        assert_self();
        if total_balance.0 > 0 {
            // There is still positive balance on the staking pool. Can't unselect the pool.
            self.set_staking_status(TransactionStatus::Idle);
            false
        } else {
            self.staking_information = None;
            true
        }
    }

    /// Called after a deposit amount was transferred out of this account to the staking pool
    /// This method needs to update staking pool status.
    pub fn on_staking_pool_deposit(&mut self, amount: WrappedBalance) -> bool {
        assert_self();

        let deposit_succeeded = is_promise_success();
        self.set_staking_status(TransactionStatus::Idle);

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

    /// Called after the given amount was requested to transfer out from the staking pool to this
    /// account.
    /// This method needs to update staking pool status.
    pub fn on_staking_pool_withdraw(&mut self, amount: WrappedBalance) -> bool {
        assert_self();

        let withdraw_succeeded = is_promise_success();
        self.set_staking_status(TransactionStatus::Idle);

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
        self.set_staking_status(TransactionStatus::Idle);

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

    /// Called after the extra amount stake was staked in the staking pool contract.
    /// This method needs to update staking pool status.
    pub fn on_staking_pool_unstake(&mut self, amount: WrappedBalance) -> bool {
        assert_self();

        let unstake_succeeded = is_promise_success();
        self.set_staking_status(TransactionStatus::Idle);

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

    /// Called after the extra amount stake was staked in the staking pool contract.
    /// This method needs to update staking pool status.
    pub fn on_voting_get_result(&mut self, #[callback] vote_index: Option<VoteIndex>) -> bool {
        assert_self();
        self.assert_transfers_disabled();

        let expected_vote_index = self
            .transfer_voting_information
            .as_ref()
            .unwrap()
            .enable_transfers_vote_index;

        if let Some(vote_index) = vote_index {
            assert_eq!(vote_index, expected_vote_index, "The enable transfers proposal has been resolved to a different vote. Transfers will never be enabled.");
            env::log(b"Transfers has been successfully enabled");
            self.transfer_voting_information = None;
            true
        } else {
            env::log(b"Voting on enabling transfers doesn't have a majority vote yet");
            false
        }
    }

    /// Called after the foundation tried to withdraw the unvested amount from the account.
    pub fn on_withdraw_unvested_amount(
        &mut self,
        amount: WrappedBalance,
        receiver_id: AccountId,
    ) -> bool {
        assert_self();

        let withdraw_succeeded = is_promise_success();
        if withdraw_succeeded {
            self.lockup_information.vesting_information = None;
            env::log(
                format!(
                    "The withdrawal of the terminated unvested amount of {} to @{} succeeded",
                    amount.0, receiver_id,
                )
                .as_bytes(),
            );
        } else {
            self.set_terminating_status(TransactionStatus::Idle);
            env::log(
                format!(
                    "The withdrawal of the terminated unvested amount of {} to @{} failed",
                    amount.0, receiver_id,
                )
                .as_bytes(),
            );
        }
        withdraw_succeeded
    }
}
