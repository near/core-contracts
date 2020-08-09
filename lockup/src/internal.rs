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
        if let ReleaseInformation::Terminating(termination_information) =
            &mut self.release_information
        {
            termination_information.status = status;
        } else {
            unreachable!("The vesting information is not at the terminating stage");
        }
    }

    pub fn assert_vesting(&self) {
        if let ReleaseInformation::Vesting(_) = &self.release_information {
            // OK
        } else {
            env::panic(b"There is no vesting in progress");
        }
    }

    pub fn assert_no_termination(&self) {
        if let ReleaseInformation::Terminating(_) = &self.release_information {
            env::panic(b"All operations are blocked until vesting termination is completed");
        }
    }

    pub fn assert_transfers_enabled(&self) {
        assert!(self.are_transfers_enabled(), "Transfers are disabled");
    }

    pub fn assert_transfers_disabled(&self) {
        assert!(
            !self.are_transfers_enabled(),
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

    pub fn assert_called_by_foundation(&self) {
        if let Some(foundation_account_id) = &self.foundation_account_id {
            assert_eq!(
                &env::predecessor_account_id(),
                foundation_account_id,
                "Can only be called by NEAR Foundation"
            )
        } else {
            env::panic(b"No NEAR Foundation account is specified in the contract");
        }
    }

    pub fn assert_owner(&self) {
        assert_eq!(
            &env::predecessor_account_id(),
            &self.owner_account_id,
            "Can only be called by the owner"
        )
    }
}
