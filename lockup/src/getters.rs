use crate::*;
use near_sdk::near_bindgen;

#[near_bindgen]
impl LockupContract {
    /// Returns the account ID of the owner.
    pub fn get_owner_account_id(&self) -> AccountId {
        self.owner_account_id.clone()
    }

    /// Returns the account ID of the selected staking pool.
    pub fn get_staking_pool_account_id(&self) -> Option<AccountId> {
        self.staking_information
            .as_ref()
            .map(|info| info.staking_pool_account_id.clone())
    }

    /// The amount of tokens that were deposited to the staking pool.
    /// NOTE: The actual balance can be larger than this known deposit balance due to staking
    /// rewards acquired on the staking pool.
    /// To refresh the amount the owner can call `refresh_staking_pool_balance`.
    pub fn get_known_deposited_balance(&self) -> WrappedBalance {
        self.staking_information
            .as_ref()
            .map(|info| info.deposit_amount.0)
            .unwrap_or(0)
            .into()
    }

    /// Returns the current termination status or `None` in case of no termination.
    pub fn get_termination_status(&self) -> Option<TerminationStatus> {
        if let ReleaseInformation::Terminating(termination_information) = &self.release_information
        {
            Some(termination_information.status)
        } else {
            None
        }
    }

    /// The amount of tokens that are not going to be vested, because the vesting schedule was
    /// terminated earlier.
    pub fn get_terminated_unvested_balance(&self) -> WrappedBalance {
        if let ReleaseInformation::Terminating(TerminationInformation {
            unvested_amount, ..
        }) = &self.release_information
        {
            *unvested_amount
        } else {
            0.into()
        }
    }

    /// The amount of tokens missing from the account balance that are required to cover
    /// the unvested balance from the early-terminated vesting schedule.
    pub fn get_terminated_unvested_balance_deficit(&self) -> WrappedBalance {
        self.get_terminated_unvested_balance()
            .0
            .saturating_sub(self.get_account_balance().0)
            .into()
    }

    /// Get the amount of tokens that are locked in this account due to lockup or vesting.
    pub fn get_locked_amount(&self) -> WrappedBalance {
        if let TransfersInformation::TransfersEnabled {
            transfers_timestamp,
        } = &self.lockup_information.transfers_information
        {
            let lockup_timestamp = std::cmp::max(
                transfers_timestamp
                    .0
                    .saturating_add(self.lockup_information.lockup_duration.0),
                self.lockup_information.lockup_timestamp.map_or(0, |t| t.0),
            );
            if lockup_timestamp <= env::block_timestamp() {
                return self.get_unvested_amount();
            }
        }
        // The entire balance is still locked before the lockup timestamp.
        self.lockup_information.lockup_amount
    }

    /// Get the amount of tokens that are already vested, but still locked due to lockup.
    pub fn get_locked_vested_amount(&self) -> WrappedBalance {
        (self.get_locked_amount().0 - self.get_unvested_amount().0).into()
    }

    /// Get the amount of tokens that are locked in this account due to vesting.
    pub fn get_unvested_amount(&self) -> WrappedBalance {
        let block_timestamp = env::block_timestamp();
        let lockup_amount = self.lockup_information.lockup_amount.0;
        match &self.release_information {
            ReleaseInformation::None => {
                // Everything is vested and unlocked
                0.into()
            }
            ReleaseInformation::Vesting(vesting_schedule) => {
                if block_timestamp < vesting_schedule.cliff_timestamp.0 {
                    // Before the cliff, nothing is vested
                    lockup_amount.into()
                } else if block_timestamp >= vesting_schedule.end_timestamp.0 {
                    // After the end, everything is vested
                    0.into()
                } else {
                    // cannot overflow since block_timestamp < vesting_schedule.end_timestamp
                    let time_left = U256::from(vesting_schedule.end_timestamp.0 - block_timestamp);
                    // The total time is positive. Checked at the contract initialization.
                    let total_time = U256::from(
                        vesting_schedule.end_timestamp.0 - vesting_schedule.start_timestamp.0,
                    );
                    let unvested_amount = U256::from(lockup_amount) * time_left / total_time;
                    // The unvested amount can't be larger than lockup_amount because the
                    // time_left is smaller than total_time.
                    unvested_amount.as_u128().into()
                }
            }
            ReleaseInformation::ReleaseDuration(release_duration) => {
                if let TransfersInformation::TransfersEnabled {
                    transfers_timestamp,
                } = &self.lockup_information.transfers_information
                {
                    let end_timestamp = transfers_timestamp.0.saturating_add(release_duration.0);
                    if block_timestamp >= end_timestamp {
                        // After the end, everything is vested
                        0.into()
                    } else {
                        let time_left = U256::from(end_timestamp - block_timestamp);
                        let unreleased_amount =
                            U256::from(lockup_amount) * time_left / U256::from(release_duration.0);
                        // The unreleased amount can't be larger than lockup_amount because the
                        // time_left is smaller than total_time.
                        unreleased_amount.as_u128().into()
                    }
                } else {
                    lockup_amount.into()
                }
            }
            ReleaseInformation::Terminating(termination_information) => {
                termination_information.unvested_amount
            }
        }
    }

    /// The balance of the account owner. It includes vested and extra tokens that may have been
    /// deposited to this account, but excludes locked tokens.
    /// NOTE: Some of this tokens may be deposited to the staking pool.
    /// This method also doesn't account for tokens locked for the contract storage.
    pub fn get_owners_balance(&self) -> WrappedBalance {
        (env::account_balance() + self.get_known_deposited_balance().0)
            .saturating_sub(self.get_locked_amount().0)
            .into()
    }

    /// Returns total balance of the account including tokens deposited on the staking pool.
    pub fn get_balance(&self) -> WrappedBalance {
        (env::account_balance() + self.get_known_deposited_balance().0).into()
    }

    /// The amount of tokens the owner can transfer now from the account.
    pub fn get_liquid_owners_balance(&self) -> WrappedBalance {
        std::cmp::min(self.get_owners_balance().0, self.get_account_balance().0).into()
    }

    /// Returns `true` if transfers are enabled, `false` otherwise.
    pub fn are_transfers_enabled(&self) -> bool {
        match &self.lockup_information.transfers_information {
            TransfersInformation::TransfersEnabled { .. } => true,
            TransfersInformation::TransfersDisabled { .. } => false,
        }
    }
}
