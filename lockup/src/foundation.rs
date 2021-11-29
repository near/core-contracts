use near_sdk::{near_bindgen, AccountId, Promise};

use crate::*;

#[near_bindgen]
impl LockupContract {
    /// FOUNDATION'S METHOD
    ///
    /// Requires 25 TGas (1 * BASE_GAS)
    ///
    /// Terminates vesting schedule and locks the remaining unvested amount.
    /// If the lockup contract was initialized with the private vesting schedule, then
    /// this method expects to receive a `VestingScheduleWithSalt` to reveal the vesting schedule,
    /// otherwise it expects `None`.
    pub fn terminate_vesting(
        &mut self,
        vesting_schedule_with_salt: Option<VestingScheduleWithSalt>,
    ) {
        self.assert_called_by_foundation();
        let vesting_schedule = self.assert_vesting(vesting_schedule_with_salt);
        let unvested_amount = self.get_unvested_amount(vesting_schedule);
        assert!(unvested_amount.0 > 0, "The account is fully vested");

        env::log(
            format!(
                "Terminating vesting. The remaining unvested balance is {}",
                unvested_amount.0
            )
            .as_bytes(),
        );

        let deficit = unvested_amount
            .0
            .saturating_sub(self.get_account_balance().0);
        // If there is deficit of liquid balance and also there is a staking pool selected, then the
        // contract will try to withdraw everything from this staking pool to cover deficit.
        let status = if deficit > 0 && self.staking_information.is_some() {
            TerminationStatus::VestingTerminatedWithDeficit
        } else {
            TerminationStatus::ReadyToWithdraw
        };

        self.vesting_information = VestingInformation::Terminating(TerminationInformation {
            unvested_amount,
            status,
        });
    }

    /// FOUNDATION'S METHOD
    ///
    /// Requires 175 TGas (7 * BASE_GAS)
    ///
    /// When the vesting is terminated and there are deficit of the tokens on the account, the
    /// deficit amount of tokens has to be unstaked and withdrawn from the staking pool.
    /// Should be invoked twice:
    /// 1. First, to unstake everything from the staking pool;
    /// 2. Second, after 4 epochs (48 hours) to prepare to withdraw.
    pub fn termination_prepare_to_withdraw(&mut self) -> Promise {
        self.assert_called_by_foundation();
        self.assert_staking_pool_is_idle();

        let status = self.get_termination_status();

        match status {
            None => {
                env::panic(b"There is no termination in progress");
            }
            Some(TerminationStatus::UnstakingInProgress)
            | Some(TerminationStatus::WithdrawingFromStakingPoolInProgress)
            | Some(TerminationStatus::WithdrawingFromAccountInProgress) => {
                env::panic(b"Another transaction is already in progress.");
            }
            Some(TerminationStatus::ReadyToWithdraw) => {
                env::panic(b"The account is ready to withdraw unvested balance.")
            }
            Some(TerminationStatus::VestingTerminatedWithDeficit) => {
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
                        &env::current_account_id(),
                        NO_DEPOSIT,
                        gas::foundation_callbacks::ON_GET_ACCOUNT_STAKED_BALANCE_TO_UNSTAKE,
                    ),
                )
            }
            Some(TerminationStatus::EverythingUnstaked) => {
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
                        &env::current_account_id(),
                        NO_DEPOSIT,
                        gas::foundation_callbacks::ON_GET_ACCOUNT_UNSTAKED_BALANCE_TO_WITHDRAW,
                    ),
                )
            }
        }
    }

    /// FOUNDATION'S METHOD
    ///
    /// Requires 75 TGas (3 * BASE_GAS)
    ///
    /// Withdraws the unvested amount from the early termination of the vesting schedule.
    pub fn termination_withdraw(&mut self, receiver_id: AccountId) -> Promise {
        self.assert_called_by_foundation();
        assert!(
            env::is_valid_account_id(receiver_id.as_bytes()),
            "The receiver account ID is invalid"
        );
        assert_eq!(
            self.get_termination_status(),
            Some(TerminationStatus::ReadyToWithdraw),
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
