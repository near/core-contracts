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

    /// The amount of tokens that were deposited to the staking pool.
    /// NOTE: The actual balance can be larger than this known deposit balance due to staking
    /// rewards acquired on the staking pool.
    pub fn get_known_deposited_balance(&self) -> WrappedBalance {
        self.staking_information
            .as_ref()
            .map(|info| info.deposit_amount.0)
            .unwrap_or(0)
            .into()
    }

    pub fn set_staking_status(&mut self, status: TransactionStatus) {
        self.staking_information
            .as_mut()
            .expect("Staking pool should be selected")
            .status = status;
    }

    pub fn set_terminating_status(&mut self, status: TransactionStatus) {
        if let Some(VestingInformation::Terminating(termination_information)) =
            self.lockup_information.vesting_information.as_mut()
        {
            termination_information.status = status;
        } else {
            unreachable!("The vesting information is not at the terminating stage");
        }
    }

    pub fn assert_no_deficit(&self) {
        assert_eq!(
            self.get_terminated_unvested_balance_deficit().0, 0,
            "All normal staking pool operations are blocked until the terminated unvested balance deficit is returned to the account"
        );
    }

    pub fn assert_transfers_enabled(&self) {
        assert!(
            self.transfer_voting_information.is_none(),
            "Transfers are disabled"
        );
    }

    pub fn assert_transfers_disabled(&self) {
        assert!(
            self.transfer_voting_information.is_some(),
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

    pub fn assert_termination_is_idle(&self) {
        if let Some(VestingInformation::Terminating(termination_information)) =
            &self.lockup_information.vesting_information
        {
            match termination_information.status {
                TransactionStatus::Idle => (),
                TransactionStatus::Busy => {
                    env::panic(b"Contract is currently busy with termination withdrawal")
                }
            };
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
