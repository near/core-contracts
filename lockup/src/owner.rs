use crate::*;
use near_sdk::{near_bindgen, AccountId, Promise, PublicKey};

#[near_bindgen]
impl LockupContract {
    /// OWNER'S METHOD
    ///
    /// Requires 75 TGas (3 * BASE_GAS)
    ///
    /// Selects staking pool contract at the given account ID. The staking pool first has to be
    /// checked against the staking pool whitelist contract.
    pub fn select_staking_pool(&mut self, staking_pool_account_id: AccountId) -> Promise {
        self.assert_owner();
        assert!(
            env::is_valid_account_id(staking_pool_account_id.as_bytes()),
            "The staking pool account ID is invalid"
        );
        self.assert_staking_pool_is_not_selected();
        self.assert_no_termination();

        env::log(
            format!(
                "Selecting staking pool @{}. Going to check whitelist first.",
                staking_pool_account_id
            )
            .as_bytes(),
        );

        ext_whitelist::is_whitelisted(
            staking_pool_account_id.clone(),
            &self.staking_pool_whitelist_account_id,
            NO_DEPOSIT,
            gas::whitelist::IS_WHITELISTED,
        )
        .then(ext_self_owner::on_whitelist_is_whitelisted(
            staking_pool_account_id,
            &env::current_account_id(),
            NO_DEPOSIT,
            gas::owner_callbacks::ON_WHITELIST_IS_WHITELISTED,
        ))
    }

    /// OWNER'S METHOD
    ///
    /// Requires 25 TGas (1 * BASE_GAS)
    ///
    /// Unselects the current staking pool.
    /// It requires that there are no known deposits left on the currently selected staking pool.
    pub fn unselect_staking_pool(&mut self) {
        self.assert_owner();
        self.assert_staking_pool_is_idle();
        self.assert_no_termination();
        // NOTE: This is best effort checks. There is still some balance might be left on the
        // staking pool, but it's up to the owner whether to unselect the staking pool.
        // The contract doesn't care about leftovers.
        assert_eq!(
            self.staking_information.as_ref().unwrap().deposit_amount.0,
            0,
            "There is still a deposit on the staking pool"
        );

        env::log(
            format!(
                "Unselected current staking pool @{}.",
                self.staking_information
                    .as_ref()
                    .unwrap()
                    .staking_pool_account_id
            )
            .as_bytes(),
        );

        self.staking_information = None;
    }

    /// OWNER'S METHOD
    ///
    /// Requires 100 TGas (4 * BASE_GAS)
    ///
    /// Deposits the given extra amount to the staking pool
    pub fn deposit_to_staking_pool(&mut self, amount: WrappedBalance) -> Promise {
        self.assert_owner();
        assert!(amount.0 > 0, "Amount should be positive");
        self.assert_staking_pool_is_idle();
        self.assert_no_termination();
        assert!(
            self.get_account_balance().0 >= amount.0,
            "The balance that can be deposited to the staking pool is lower than the extra amount"
        );

        env::log(
            format!(
                "Depositing {} to the staking pool @{}",
                amount.0,
                self.staking_information
                    .as_ref()
                    .unwrap()
                    .staking_pool_account_id
            )
            .as_bytes(),
        );

        self.set_staking_pool_status(TransactionStatus::Busy);

        ext_staking_pool::deposit(
            &self
                .staking_information
                .as_ref()
                .unwrap()
                .staking_pool_account_id,
            amount.0,
            gas::staking_pool::DEPOSIT,
        )
        .then(ext_self_owner::on_staking_pool_deposit(
            amount,
            &env::current_account_id(),
            NO_DEPOSIT,
            gas::owner_callbacks::ON_STAKING_POOL_DEPOSIT,
        ))
    }

    /// OWNER'S METHOD
    ///
    /// Requires 125 TGas (5 * BASE_GAS)
    ///
    /// Deposits and stakes the given extra amount to the selected staking pool
    pub fn deposit_and_stake(&mut self, amount: WrappedBalance) -> Promise {
        self.assert_owner();
        assert!(amount.0 > 0, "Amount should be positive");
        self.assert_staking_pool_is_idle();
        self.assert_no_termination();
        assert!(
            self.get_account_balance().0 >= amount.0,
            "The balance that can be deposited to the staking pool is lower than the extra amount"
        );

        env::log(
            format!(
                "Depositing and staking {} to the staking pool @{}",
                amount.0,
                self.staking_information
                    .as_ref()
                    .unwrap()
                    .staking_pool_account_id
            )
            .as_bytes(),
        );

        self.set_staking_pool_status(TransactionStatus::Busy);

        ext_staking_pool::deposit_and_stake(
            &self
                .staking_information
                .as_ref()
                .unwrap()
                .staking_pool_account_id,
            amount.0,
            gas::staking_pool::DEPOSIT_AND_STAKE,
        )
        .then(ext_self_owner::on_staking_pool_deposit_and_stake(
            amount,
            &env::current_account_id(),
            NO_DEPOSIT,
            gas::owner_callbacks::ON_STAKING_POOL_DEPOSIT_AND_STAKE,
        ))
    }

    /// OWNER'S METHOD
    ///
    /// Requires 75 TGas (3 * BASE_GAS)
    ///
    /// Retrieves total balance from the staking pool and remembers it internally.
    /// This method is helpful when the owner received some rewards for staking and wants to
    /// transfer them back to this account for withdrawal. In order to know the actual liquid
    /// balance on the account, this contract needs to query the staking pool.
    pub fn refresh_staking_pool_balance(&mut self) -> Promise {
        self.assert_owner();
        self.assert_staking_pool_is_idle();
        self.assert_no_termination();

        env::log(
            format!(
                "Fetching total balance from the staking pool @{}",
                self.staking_information
                    .as_ref()
                    .unwrap()
                    .staking_pool_account_id
            )
            .as_bytes(),
        );

        self.set_staking_pool_status(TransactionStatus::Busy);

        ext_staking_pool::get_account_total_balance(
            env::current_account_id(),
            &self
                .staking_information
                .as_ref()
                .unwrap()
                .staking_pool_account_id,
            NO_DEPOSIT,
            gas::staking_pool::GET_ACCOUNT_TOTAL_BALANCE,
        )
        .then(ext_self_owner::on_get_account_total_balance(
            &env::current_account_id(),
            NO_DEPOSIT,
            gas::owner_callbacks::ON_GET_ACCOUNT_TOTAL_BALANCE,
        ))
    }

    /// OWNER'S METHOD
    ///
    /// Requires 125 TGas (5 * BASE_GAS)
    ///
    /// Withdraws the given amount from the staking pool
    pub fn withdraw_from_staking_pool(&mut self, amount: WrappedBalance) -> Promise {
        self.assert_owner();
        assert!(amount.0 > 0, "Amount should be positive");
        self.assert_staking_pool_is_idle();
        self.assert_no_termination();

        env::log(
            format!(
                "Withdrawing {} from the staking pool @{}",
                amount.0,
                self.staking_information
                    .as_ref()
                    .unwrap()
                    .staking_pool_account_id
            )
            .as_bytes(),
        );

        self.set_staking_pool_status(TransactionStatus::Busy);

        ext_staking_pool::withdraw(
            amount,
            &self
                .staking_information
                .as_ref()
                .unwrap()
                .staking_pool_account_id,
            NO_DEPOSIT,
            gas::staking_pool::WITHDRAW,
        )
        .then(ext_self_owner::on_staking_pool_withdraw(
            amount,
            &env::current_account_id(),
            NO_DEPOSIT,
            gas::owner_callbacks::ON_STAKING_POOL_WITHDRAW,
        ))
    }

    /// OWNER'S METHOD
    ///
    /// Requires 175 TGas (7 * BASE_GAS)
    ///
    /// Tries to withdraws all unstaked balance from the staking pool
    pub fn withdraw_all_from_staking_pool(&mut self) -> Promise {
        self.assert_owner();
        self.assert_staking_pool_is_idle();
        self.assert_no_termination();

        env::log(
            format!(
                "Going to query the unstaked balance at the staking pool @{}",
                self.staking_information
                    .as_ref()
                    .unwrap()
                    .staking_pool_account_id
            )
            .as_bytes(),
        );

        self.set_staking_pool_status(TransactionStatus::Busy);

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
            ext_self_owner::on_get_account_unstaked_balance_to_withdraw_by_owner(
                &env::current_account_id(),
                NO_DEPOSIT,
                gas::owner_callbacks::ON_GET_ACCOUNT_UNSTAKED_BALANCE_TO_WITHDRAW_BY_OWNER,
            ),
        )
    }

    /// OWNER'S METHOD
    ///
    /// Requires 125 TGas (5 * BASE_GAS)
    ///
    /// Stakes the given extra amount at the staking pool
    pub fn stake(&mut self, amount: WrappedBalance) -> Promise {
        self.assert_owner();
        assert!(amount.0 > 0, "Amount should be positive");
        self.assert_staking_pool_is_idle();
        self.assert_no_termination();

        env::log(
            format!(
                "Staking {} at the staking pool @{}",
                amount.0,
                self.staking_information
                    .as_ref()
                    .unwrap()
                    .staking_pool_account_id
            )
            .as_bytes(),
        );

        self.set_staking_pool_status(TransactionStatus::Busy);

        ext_staking_pool::stake(
            amount,
            &self
                .staking_information
                .as_ref()
                .unwrap()
                .staking_pool_account_id,
            NO_DEPOSIT,
            gas::staking_pool::STAKE,
        )
        .then(ext_self_owner::on_staking_pool_stake(
            amount,
            &env::current_account_id(),
            NO_DEPOSIT,
            gas::owner_callbacks::ON_STAKING_POOL_STAKE,
        ))
    }

    /// OWNER'S METHOD
    ///
    /// Requires 125 TGas (5 * BASE_GAS)
    ///
    /// Unstakes the given amount at the staking pool
    pub fn unstake(&mut self, amount: WrappedBalance) -> Promise {
        self.assert_owner();
        assert!(amount.0 > 0, "Amount should be positive");
        self.assert_staking_pool_is_idle();
        self.assert_no_termination();

        env::log(
            format!(
                "Unstaking {} from the staking pool @{}",
                amount.0,
                self.staking_information
                    .as_ref()
                    .unwrap()
                    .staking_pool_account_id
            )
            .as_bytes(),
        );

        self.set_staking_pool_status(TransactionStatus::Busy);

        ext_staking_pool::unstake(
            amount,
            &self
                .staking_information
                .as_ref()
                .unwrap()
                .staking_pool_account_id,
            NO_DEPOSIT,
            gas::staking_pool::UNSTAKE,
        )
        .then(ext_self_owner::on_staking_pool_unstake(
            amount,
            &env::current_account_id(),
            NO_DEPOSIT,
            gas::owner_callbacks::ON_STAKING_POOL_UNSTAKE,
        ))
    }

    /// OWNER'S METHOD
    ///
    /// Requires 125 TGas (5 * BASE_GAS)
    ///
    /// Unstakes all tokens from the staking pool
    pub fn unstake_all(&mut self) -> Promise {
        self.assert_owner();
        self.assert_staking_pool_is_idle();
        self.assert_no_termination();

        env::log(
            format!(
                "Unstaking all tokens from the staking pool @{}",
                self.staking_information
                    .as_ref()
                    .unwrap()
                    .staking_pool_account_id
            )
            .as_bytes(),
        );

        self.set_staking_pool_status(TransactionStatus::Busy);

        ext_staking_pool::unstake_all(
            &self
                .staking_information
                .as_ref()
                .unwrap()
                .staking_pool_account_id,
            NO_DEPOSIT,
            gas::staking_pool::UNSTAKE_ALL,
        )
        .then(ext_self_owner::on_staking_pool_unstake_all(
            &env::current_account_id(),
            NO_DEPOSIT,
            gas::owner_callbacks::ON_STAKING_POOL_UNSTAKE_ALL,
        ))
    }

    /// OWNER'S METHOD
    ///
    /// Requires 75 TGas (3 * BASE_GAS)
    /// Not intended to hand over the access to someone else except the owner
    ///
    /// Calls voting contract to validate if the transfers were enabled by voting. Once transfers
    /// are enabled, they can't be disabled anymore.
    pub fn check_transfers_vote(&mut self) -> Promise {
        self.assert_owner();
        self.assert_transfers_disabled();
        self.assert_no_termination();

        let transfer_poll_account_id = match &self.lockup_information.transfers_information {
            TransfersInformation::TransfersDisabled {
                transfer_poll_account_id,
            } => transfer_poll_account_id,
            _ => unreachable!(),
        };

        env::log(
            format!(
                "Checking that transfers are enabled at the transfer poll contract @{}",
                transfer_poll_account_id,
            )
            .as_bytes(),
        );

        ext_transfer_poll::get_result(
            &transfer_poll_account_id,
            NO_DEPOSIT,
            gas::transfer_poll::GET_RESULT,
        )
        .then(ext_self_owner::on_get_result_from_transfer_poll(
            &env::current_account_id(),
            NO_DEPOSIT,
            gas::owner_callbacks::ON_VOTING_GET_RESULT,
        ))
    }

    /// OWNER'S METHOD
    ///
    /// Requires 50 TGas (2 * BASE_GAS)
    /// Not intended to hand over the access to someone else except the owner
    ///
    /// Transfers the given amount to the given receiver account ID.
    /// This requires transfers to be enabled within the voting contract.
    pub fn transfer(&mut self, amount: WrappedBalance, receiver_id: AccountId) -> Promise {
        self.assert_owner();
        assert!(amount.0 > 0, "Amount should be positive");
        assert!(
            env::is_valid_account_id(receiver_id.as_bytes()),
            "The receiver account ID is invalid"
        );
        self.assert_transfers_enabled();
        self.assert_no_staking_or_idle();
        self.assert_no_termination();
        assert!(
            self.get_liquid_owners_balance().0 >= amount.0,
            "The available liquid balance {} is smaller than the requested transfer amount {}",
            self.get_liquid_owners_balance().0,
            amount.0,
        );

        env::log(format!("Transferring {} to account @{}", amount.0, receiver_id).as_bytes());

        Promise::new(receiver_id).transfer(amount.0)
    }

    /// OWNER'S METHOD
    ///
    /// Requires 50 TGas (2 * BASE_GAS)
    /// Not intended to hand over the access to someone else except the owner
    ///
    /// Adds full access key with the given public key to the account.
    /// The following requirements should be met:
    /// - The contract is fully vested;
    /// - Lockup duration has expired;
    /// - Transfers are enabled;
    /// - If there’s a termination made by foundation, it has to be finished.
    /// Full access key will allow owner to use this account as a regular account and remove
    /// the contract.
    pub fn add_full_access_key(&mut self, new_public_key: Base58PublicKey) -> Promise {
        self.assert_owner();
        self.assert_transfers_enabled();
        self.assert_no_staking_or_idle();
        self.assert_no_termination();
        assert_eq!(self.get_locked_amount().0, 0, "Tokens are still locked/unvested");

        env::log(b"Adding a full access key");

        let new_public_key: PublicKey = new_public_key.into();

        Promise::new(env::current_account_id()).add_full_access_key(new_public_key)
    }
}
