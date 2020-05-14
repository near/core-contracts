use crate::*;
use near_sdk::{near_bindgen, AccountId, Promise};

#[near_bindgen]
impl LockupContract {
    /************************/
    /* Foundation's Methods */
    /************************/

    /// FOUNDATION'S METHOD
    /// Terminates vesting schedule and locks the remaining unvested amount.
    pub fn terminate_vesting(&mut self) {
        assert_self();
        assert_eq!(
            self.get_terminated_unvested_balance().0,
            0,
            "Vesting has been already terminated"
        );
        let unvested_amount = self.get_unvested_amount();
        assert!(unvested_amount.0 > 0, "The account is fully vested");

        env::log(
            format!(
                "Terminating vesting. The remaining unvested balance is {}",
                unvested_amount.0
            )
            .as_bytes(),
        );

        let status = if self.get_terminated_unvested_balance_deficit().0 > 0
            && self.get_known_deposited_balance().0 > 0
        {
            TerminationStatus::VestingTerminatedWithDeficit
        } else {
            TerminationStatus::ReadyToWithdraw
        };

        self.lockup_information.vesting_information =
            Some(VestingInformation::Terminating(TerminationInformation {
                unvested_amount,
                status,
            }));
    }

    /// FOUNDATION'S METHOD
    /// When the vesting is terminated and there are deficit of the tokens on the account, the
    /// deficit amount of tokens has to be unstaked and withdrawn from the staking pool.
    pub fn termination_prepare_to_withdraw(&mut self) -> Promise {
        assert_self();
        self.assert_staking_pool_is_idle();

        let status = self.get_termination_status();

        match status {
            TerminationStatus::UnstakingInProgress
            | TerminationStatus::WithdrawingFromStakingPoolInProgress
            | TerminationStatus::WithdrawingFromAccountInProgress => {
                env::panic(b"Another transaction is already in progress.");
            }
            TerminationStatus::ReadyToWithdraw => {
                env::panic(b"The account is ready to withdraw unvested balance.")
            }
            TerminationStatus::VestingTerminatedWithDeficit => {
                // Need to unstake
                self.set_termination_status(TerminationStatus::UnstakingInProgress);
                self.set_staking_pool_status(TransactionStatus::Busy);
                env::log(b"Termination Step: Going to unstake everything from the staking pool");

                ext_staking_pool::get_account_staked_balance(
                    env::current_account_id(),
                    &self
                        .staking_information
                        .as_ref()
                        .unwrap()
                        .staking_pool_account_id,
                    NO_DEPOSIT,
                    gas::staking_pool::GET_ACCOUNT_STAKED_BALANCE,
                )
                .then(
                    ext_self_foundation::on_get_account_staked_balance_to_unstake(
                        &self
                            .staking_information
                            .as_ref()
                            .unwrap()
                            .staking_pool_account_id,
                        NO_DEPOSIT,
                        gas::foundation_callbacks::ON_GET_ACCOUNT_STAKED_BALANCE_TO_UNSTAKE,
                    ),
                )
            }
            TerminationStatus::EverythingUnstaked => {
                // Need to withdraw everything
                self.set_termination_status(
                    TerminationStatus::WithdrawingFromStakingPoolInProgress,
                );
                self.set_staking_pool_status(TransactionStatus::Busy);
                env::log(b"Termination Step: Going to withdraw everything from the staking pool");

                ext_staking_pool::get_account_unstaked_balance(
                    env::current_account_id(),
                    &self
                        .staking_information
                        .as_ref()
                        .unwrap()
                        .staking_pool_account_id,
                    NO_DEPOSIT,
                    gas::staking_pool::GET_ACCOUNT_UNSTAKED_BALANCE,
                )
                .then(
                    ext_self_foundation::on_get_account_unstaked_balance_to_withdraw(
                        &self
                            .staking_information
                            .as_ref()
                            .unwrap()
                            .staking_pool_account_id,
                        NO_DEPOSIT,
                        gas::foundation_callbacks::ON_GET_ACCOUNT_UNSTAKED_BALANCE_TO_WITHDRAW,
                    ),
                )
            }
        }
    }

    /// FOUNDATION'S METHOD
    /// Withdraws the unvested amount from the early termination of the vesting schedule.
    pub fn withdraw_unvested_amount(&mut self, receiver_id: AccountId) -> Promise {
        assert_self();
        assert!(
            env::is_valid_account_id(receiver_id.as_bytes()),
            "The receiver account ID is invalid"
        );
        assert_eq!(
            self.get_termination_status(),
            TerminationStatus::ReadyToWithdraw,
            "Termination status is not ready to withdraw"
        );

        let amount = std::cmp::min(
            self.get_terminated_unvested_balance().0,
            self.get_account_balance().0,
        );
        assert!(
            amount > 0,
            "The account doesn't have enough liquid balance to withdraw any amount"
        );

        env::log(
            format!(
                "Termination Step: Withdrawing {} of terminated unvested balance to account @{}",
                amount, receiver_id
            )
            .as_bytes(),
        );

        self.set_termination_status(TerminationStatus::WithdrawingFromAccountInProgress);

        Promise::new(receiver_id.clone()).transfer(amount).then(
            ext_self_foundation::on_withdraw_unvested_amount(
                amount.into(),
                receiver_id,
                &env::current_account_id(),
                NO_DEPOSIT,
                gas::foundation_callbacks::ON_WITHDRAW_UNVESTED_AMOUNT,
            ),
        )
    }
}
