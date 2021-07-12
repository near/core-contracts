use near_sdk::json_types::U128;
use near_sdk::near_bindgen;

use crate::*;

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

    /// Returns the amount of tokens that were deposited to the staking pool.
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
        if let VestingInformation::Terminating(termination_information) = &self.vesting_information
        {
            Some(termination_information.status)
        } else {
            None
        }
    }

    /// Returns the amount of tokens that are not going to be vested, because the vesting schedule
    /// was terminated earlier.
    pub fn get_terminated_unvested_balance(&self) -> WrappedBalance {
        if let VestingInformation::Terminating(TerminationInformation {
            unvested_amount, ..
        }) = &self.vesting_information
        {
            *unvested_amount
        } else {
            0.into()
        }
    }

    /// Returns the amount of tokens missing from the account balance that are required to cover
    /// the unvested balance from the early-terminated vesting schedule.
    pub fn get_terminated_unvested_balance_deficit(&self) -> WrappedBalance {
        self.get_terminated_unvested_balance()
            .0
            .saturating_sub(self.get_account_balance().0)
            .into()
    }

    /// Returns the amount of tokens that are locked in the account due to lockup or vesting.
    pub fn get_locked_amount(&self) -> WrappedBalance {
        let lockup_amount = self.lockup_information.lockup_amount;
        if let TransfersInformation::TransfersEnabled {
            transfers_timestamp,
        } = &self.lockup_information.transfers_information
        {
            let lockup_timestamp = std::cmp::max(
                transfers_timestamp
                    .0
                    .saturating_add(self.lockup_information.lockup_duration),
                self.lockup_information.lockup_timestamp.unwrap_or(0),
            );
            let block_timestamp = env::block_timestamp();
            if lockup_timestamp <= block_timestamp {
                let unreleased_amount =
                    if let &Some(release_duration) = &self.lockup_information.release_duration {
                        let end_timestamp = lockup_timestamp.saturating_add(release_duration);
                        if block_timestamp >= end_timestamp {
                            // Everything is released
                            0
                        } else {
                            let time_left = U256::from(end_timestamp - block_timestamp);
                            let unreleased_amount = U256::from(lockup_amount) * time_left
                                / U256::from(release_duration);
                            // The unreleased amount can't be larger than lockup_amount because the
                            // time_left is smaller than total_time.
                            unreleased_amount.as_u128()
                        }
                    } else {
                        0
                    };

                let unvested_amount = match &self.vesting_information {
                    VestingInformation::VestingSchedule(vs) => self.get_unvested_amount(vs.clone()),
                    VestingInformation::Terminating(terminating) => terminating.unvested_amount,
                    // Vesting is private, so we can assume the vesting started before lockup date.
                    _ => U128(0),
                };
                return std::cmp::max(
                    unreleased_amount
                        .saturating_sub(self.lockup_information.termination_withdrawn_tokens),
                    unvested_amount.0,
                )
                .into();
            }
        }
        // The entire balance is still locked before the lockup timestamp.
        (lockup_amount - self.lockup_information.termination_withdrawn_tokens).into()
    }

    /// Returns the amount of tokens that are already vested, but still locked due to lockup.
    /// Takes raw vesting schedule, in case the internal vesting schedule is private.
    pub fn get_locked_vested_amount(&self, vesting_schedule: VestingSchedule) -> WrappedBalance {
        (self.get_locked_amount().0 - self.get_unvested_amount(vesting_schedule).0).into()
    }

    /// Returns the amount of tokens that are locked in this account due to vesting schedule.
    /// Takes raw vesting schedule, in case the internal vesting schedule is private.
    pub fn get_unvested_amount(&self, vesting_schedule: VestingSchedule) -> WrappedBalance {
        let block_timestamp = env::block_timestamp();
        let lockup_amount = self.lockup_information.lockup_amount;
        match &self.vesting_information {
            VestingInformation::Terminating(termination_information) => {
                termination_information.unvested_amount
            }
            VestingInformation::None => U128::from(0),
            _ => {
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
        }
    }

    /// Returns the internal vesting information.
    pub fn get_vesting_information(&self) -> VestingInformation {
        self.vesting_information.clone()
    }

    /// Returns the balance of the account owner. It includes vested and extra tokens that
    /// may have been deposited to this account, but excludes locked tokens.
    /// NOTE: Some of this tokens may be deposited to the staking pool.
    /// This method also doesn't account for tokens locked for the contract storage.
    pub fn get_owners_balance(&self) -> WrappedBalance {
        (env::account_balance() + self.get_known_deposited_balance().0)
            .saturating_sub(self.get_locked_amount().0)
            .into()
    }

    /// Returns total balance of the account including tokens deposited to the staking pool.
    pub fn get_balance(&self) -> WrappedBalance {
        (env::account_balance() + self.get_known_deposited_balance().0).into()
    }

    /// Returns the amount of tokens the owner can transfer from the account.
    /// Transfers have to be enabled.
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
