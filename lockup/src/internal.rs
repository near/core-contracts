use crate::*;

/********************/
/* Internal methods */
/********************/

impl LockupContract {
    /// The balance of the account excluding the storage staking balance.
    /// NOTE: The storage staking balance can't be transferred out without deleting this contract.
    pub fn get_account_balance(&self) -> WrappedBalance {
        env::account_balance()
            .saturating_sub(MIN_BALANCE_FOR_STORAGE)
            .into()
    }

    pub fn set_staking_pool_status(&mut self, status: TransactionStatus) {
        self.staking_information
            .as_mut()
            .expect("Staking pool should be selected")
            .status = status;
    }

    pub fn set_termination_status(&mut self, status: TerminationStatus) {
        if let Some(VestingInformation::Terminating(termination_information)) =
            self.lockup_information.vesting_information.as_mut()
        {
            termination_information.status = status;
        } else {
            unreachable!("The vesting information is not at the terminating stage");
        }
    }

    pub fn assert_vesting(&self) {
        if let Some(VestingInformation::Vesting(_)) = &self.lockup_information.vesting_information {
            // OK
        } else {
            env::panic(b"There is no vesting in progress");
        }
    }

    pub fn assert_no_termination(&self) {
        if let Some(VestingInformation::Terminating(_)) =
            &self.lockup_information.vesting_information
        {
            env::panic(b"All operations are blocked until vesting termination is completed");
        }
    }

    pub fn assert_transfers_enabled(&self) {
        assert!(
            self.transfer_poll_account_id.is_none(),
            "Transfers are disabled"
        );
    }

    pub fn assert_transfers_disabled(&self) {
        assert!(
            self.transfer_poll_account_id.is_some(),
            "Transfers are already enabled"
        );
    }

    pub fn assert_no_staking_or_idle(&self) {
        if let Some(staking_information) = &self.staking_information {
            match staking_information.status {
                TransactionStatus::Idle => (),
                TransactionStatus::Busy => {
                    env::panic(b"Contract is currently busy with another operation")
                }
            };
        }
    }

    pub fn get_termination_status(&self) -> TerminationStatus {
        if let Some(VestingInformation::Terminating(termination_information)) =
            &self.lockup_information.vesting_information
        {
            termination_information.status
        } else {
            env::panic(b"There are no termination in progress");
        }
    }

    pub fn assert_staking_pool_is_idle(&self) {
        assert!(
            self.staking_information.is_some(),
            "Staking pool is not selected"
        );
        match self.staking_information.as_ref().unwrap().status {
            TransactionStatus::Idle => (),
            TransactionStatus::Busy => {
                env::panic(b"Contract is currently busy with another operation")
            }
        };
    }

    pub fn assert_staking_pool_is_not_selected(&self) {
        assert!(
            self.staking_information.is_none(),
            "Staking pool is already selected"
        );
    }
}
