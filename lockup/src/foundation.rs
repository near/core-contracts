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

        self.lockup_information.vesting_information =
            Some(VestingInformation::Terminating(TerminationInformation {
                unvested_amount,
                status: TransactionStatus::Idle,
            }));
    }
    //
    // /// FOUNDATION'S METHOD
    // /// When the vesting is terminated and there are deficit of the tokens on the account, the
    // /// deficit amount of tokens has to be unstaked and withdrawn from the staking pool.
    // pub fn resolve_deficit(&mut self) -> Promise {
    //     assert_self();
    //     self.assert_staking_pool_is_idle();
    //     self.assert_termination_is_idle();
    //
    //     let deficit = self.get_terminated_unvested_balance_deficit().0;
    //     assert!(deficit > 0, "There are no unvested balance deficit");
    //
    //     let unstaked_balance = self.get_known_unstaked_balance().0;
    //
    //     if unstaked_balance < deficit {
    //         let need_to_unstake = deficit - unstaked_balance;
    //         env::log(
    //             format!(
    //                 "Trying to unstake {} to be able to withdraw termination unvested balance deficit of {}",
    //                 need_to_unstake,
    //                 deficit,
    //             )
    //                 .as_bytes(),
    //         );
    //         self.unstake(need_to_unstake.into())
    //     } else {
    //         env::log(
    //             format!(
    //                 "Trying to withdraw {} to cover the termination unvested balance deficit",
    //                 deficit
    //             )
    //             .as_bytes(),
    //         );
    //
    //         self.withdraw_from_staking_pool(deficit.into())
    //     }
    // }

    /// FOUNDATION'S METHOD
    /// Withdraws the unvested amount from the early termination of the vesting schedule.
    pub fn withdraw_unvested_amount(&mut self, receiver_id: AccountId) -> Promise {
        assert_self();
        assert!(
            env::is_valid_account_id(receiver_id.as_bytes()),
            "The receiver account ID is invalid"
        );
        self.assert_termination_is_idle();

        let amount = self.get_terminated_unvested_balance();
        assert!(
            self.get_account_balance().0 >= amount.0,
            "The account doesn't have enough balance to withdraw the unvested amount"
        );

        env::log(
            format!(
                "Withdrawing {} terminated unvested balance to account @{}",
                amount.0, receiver_id
            )
            .as_bytes(),
        );

        self.set_terminating_status(TransactionStatus::Busy);

        Promise::new(receiver_id.clone()).transfer(amount.0).then(
            ext_self::on_withdraw_unvested_amount(
                amount,
                receiver_id,
                &env::current_account_id(),
                NO_DEPOSIT,
                gas::callbacks::ON_WITHDRAW_UNVESTED_AMOUNT,
            ),
        )
    }
}
