use crate::*;
use near_sdk::{near_bindgen, AccountId, Promise};

#[near_bindgen]
impl LockupContract {
    /// OWNER'S METHOD
    /// Selects staking pool contract at the given account ID. The staking pool first has to be
    /// checked against the staking pool whitelist contract.
    pub fn select_staking_pool(&mut self, staking_pool_account_id: AccountId) -> Promise {
        assert_self();
        assert!(
            env::is_valid_account_id(staking_pool_account_id.as_bytes()),
            "The staking pool account ID is invalid"
        );
        self.assert_staking_pool_is_not_selected();

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
        .then(ext_self::on_whitelist_is_whitelisted(
            staking_pool_account_id,
            &env::current_account_id(),
            NO_DEPOSIT,
            gas::callbacks::ON_WHITELIST_IS_WHITELISTED,
        ))
    }

    /// OWNER'S METHOD
    /// Unselects the current staking pool.
    /// It requires that there are no known deposits left on the currently selected staking pool.
    pub fn unselect_staking_pool(&mut self) {
        assert_self();
        self.assert_staking_pool_is_idle();
        self.assert_no_deficit();
        // NOTE: This is best effort checks. There is still some balance might be left on the
        // staking pool, but it's on the owner. The contract doesn't care about leftover.
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
    /// Deposits the given extra amount to the staking pool
    pub fn deposit_to_staking_pool(&mut self, amount: WrappedBalance) -> Promise {
        assert_self();
        assert!(amount.0 > 0, "Amount should be positive");
        self.assert_staking_pool_is_idle();
        self.assert_no_deficit();
        assert!(
            self.get_liquid_balance().0 >= amount.0,
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

        self.set_staking_status(TransactionStatus::Busy);

        ext_staking_pool::deposit(
            &self
                .staking_information
                .as_ref()
                .unwrap()
                .staking_pool_account_id,
            amount.0,
            gas::staking_pool::DEPOSIT,
        )
        .then(ext_self::on_staking_pool_deposit(
            amount,
            &env::current_account_id(),
            NO_DEPOSIT,
            gas::callbacks::ON_STAKING_POOL_DEPOSIT,
        ))
    }

    /// OWNER'S METHOD
    /// Withdraws the given amount from the staking pool
    pub fn withdraw_from_staking_pool(&mut self, amount: WrappedBalance) -> Promise {
        assert_self();
        assert!(amount.0 > 0, "Amount should be positive");
        self.assert_staking_pool_is_idle();
        self.assert_no_deficit();

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

        self.set_staking_status(TransactionStatus::Busy);

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
        .then(ext_self::on_staking_pool_withdraw(
            amount,
            &env::current_account_id(),
            NO_DEPOSIT,
            gas::callbacks::ON_STAKING_POOL_WITHDRAW,
        ))
    }

    /// OWNER'S METHOD
    /// Stakes the given extra amount at the staking pool
    pub fn stake(&mut self, amount: WrappedBalance) -> Promise {
        assert_self();
        assert!(amount.0 > 0, "Amount should be positive");
        self.assert_staking_pool_is_idle();
        self.assert_no_deficit();

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

        self.set_staking_status(TransactionStatus::Busy);

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
        .then(ext_self::on_staking_pool_stake(
            amount,
            &env::current_account_id(),
            NO_DEPOSIT,
            gas::callbacks::ON_STAKING_POOL_STAKE,
        ))
    }

    /// OWNER'S METHOD
    /// Unstakes the given amount at the staking pool
    pub fn unstake(&mut self, amount: WrappedBalance) -> Promise {
        assert_self();
        assert!(amount.0 > 0, "Amount should be positive");
        self.assert_staking_pool_is_idle();
        self.assert_no_deficit();

        env::log(
            format!(
                "Unstaking {} at the staking pool @{}",
                amount.0,
                self.staking_information
                    .as_ref()
                    .unwrap()
                    .staking_pool_account_id
            )
            .as_bytes(),
        );

        self.set_staking_status(TransactionStatus::Busy);

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
        .then(ext_self::on_staking_pool_unstake(
            amount,
            &env::current_account_id(),
            NO_DEPOSIT,
            gas::callbacks::ON_STAKING_POOL_UNSTAKE,
        ))
    }

    /// OWNER'S METHOD
    /// Calls voting contract to validate if the transfers were enabled by voting. Once transfers
    /// are enabled, they can't be disabled anymore.
    pub fn check_transfers_vote(&mut self) -> Promise {
        assert_self();
        self.assert_transfers_disabled();

        let transfer_voting_information = self.transfer_voting_information.as_ref().unwrap();

        env::log(
            format!(
                "Checking that transfers are enabled (proposal {}) at the voting contract @{}",
                transfer_voting_information.transfer_proposal_id,
                transfer_voting_information.voting_contract_account_id,
            )
            .as_bytes(),
        );

        ext_voting::get_result(
            transfer_voting_information.transfer_proposal_id,
            &transfer_voting_information.voting_contract_account_id,
            NO_DEPOSIT,
            gas::voting::GET_RESULT,
        )
        .then(ext_self::on_voting_get_result(
            &env::current_account_id(),
            NO_DEPOSIT,
            gas::callbacks::ON_VOTING_GET_RESULT,
        ))
    }

    /// OWNER'S METHOD
    /// Transfers the given extra amount to the given receiver account ID.
    /// This requires transfers to be enabled within the voting contract.
    pub fn transfer(&mut self, amount: WrappedBalance, receiver_id: AccountId) -> Promise {
        assert_self();
        assert!(amount.0 > 0, "Amount should be positive");
        assert!(
            env::is_valid_account_id(receiver_id.as_bytes()),
            "The receiver account ID is invalid"
        );
        self.assert_transfers_enabled();
        self.assert_no_staking_or_idle();
        assert!(
            self.get_liquid_owners_balance().0 >= amount.0,
            "The available liquid balance is smaller than the requested transfer amount"
        );

        env::log(format!("Transferring {} to account @{}", amount.0, receiver_id).as_bytes());

        Promise::new(receiver_id).transfer(amount.0)
    }
}
