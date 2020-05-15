pub mod whitelist {
    /// Gas attached to the promise to check whether the given staking pool Account ID is
    /// whitelisted.
    /// Requires 100e12 (no external calls).
    pub const IS_WHITELISTED: u64 = 100_000_000_000_000;
}

pub mod staking_pool {
    /// Gas attached to deposit call on the staking pool contract.
    /// Requires 100e12 for local updates + 200e12 potentially restake.
    pub const DEPOSIT: u64 = 300_000_000_000_000;

    /// Gas attached to withdraw call on the staking pool contract.
    /// Requires 100e12 for execution + 200e12 for transferring amount to us and potentially restake.
    pub const WITHDRAW: u64 = 300_000_000_000_000;

    /// Gas attached to stake call on the staking pool contract.
    /// Requires 100e12 for execution + 200e12 for staking call.
    pub const STAKE: u64 = 300_000_000_000_000;

    /// Gas attached to unstake call on the staking pool contract.
    /// Requires 100e12 for execution + 200e12 for staking call.
    pub const UNSTAKE: u64 = 300_000_000_000_000;

    /// The amount of gas required to get the current staked balance of this account from the
    /// staking pool.
    /// Requires 100e12 for local processing.
    pub const GET_ACCOUNT_STAKED_BALANCE: u64 = 100_000_000_000_000;

    /// The amount of gas required to get current unstaked balance of this account from the
    /// staking pool.
    /// Requires 100e12 for local processing.
    pub const GET_ACCOUNT_UNSTAKED_BALANCE: u64 = 100_000_000_000_000;
}

pub mod transfer_poll {
    /// Gas attached to the promise to check whether transfers were enabled on the transfer poll
    /// contract.
    /// Requires 100e12 (no external calls).
    pub const GET_RESULT: u64 = 100_000_000_000_000;
}

pub mod owner_callbacks {
    /// Gas attached to the inner callback for processing whitelist check results.
    /// Requires 100e12 for local execution.
    pub const ON_WHITELIST_IS_WHITELISTED: u64 = 100_000_000_000_000;

    /// Gas attached to the inner callback for processing result of the deposit call to the
    /// staking pool.
    /// Requires 100e12 for local updates.
    pub const ON_STAKING_POOL_DEPOSIT: u64 = 100_000_000_000_000;

    /// Gas attached to the inner callback for processing result of the withdraw call to the
    /// staking pool.
    /// Requires 100e12 for local updates.
    pub const ON_STAKING_POOL_WITHDRAW: u64 = 100_000_000_000_000;

    /// Gas attached to the inner callback for processing result of the stake call to the
    /// staking pool.
    pub const ON_STAKING_POOL_STAKE: u64 = 100_000_000_000_000;

    /// Gas attached to the inner callback for processing result of the unstake call  to the
    /// staking pool.
    /// Requires 100e12 for local updates.
    pub const ON_STAKING_POOL_UNSTAKE: u64 = 100_000_000_000_000;

    /// Gas attached to the inner callback for processing result of the checking result for
    /// transfer voting call to the voting contract.
    /// Requires 100e12 for local updates.
    pub const ON_VOTING_GET_RESULT: u64 = 100_000_000_000_000;
}

pub mod foundation_callbacks {
    /// Gas attached to the inner callback for processing result of the call to get the current
    /// staked balance from the staking pool.
    /// The callback might proceed with unstaking.
    /// Requires 100e12 for local updates + gas for unstake + gas for another callback.
    pub const ON_GET_ACCOUNT_STAKED_BALANCE_TO_UNSTAKE: u64 = 100_000_000_000_000
        + crate::gas::staking_pool::UNSTAKE
        + ON_STAKING_POOL_UNSTAKE_FOR_TERMINATION;

    /// Gas attached to the inner callback for processing result of the unstake call  to the
    /// staking pool.
    /// Requires 100e12 for local updates.
    pub const ON_STAKING_POOL_UNSTAKE_FOR_TERMINATION: u64 = 100_000_000_000_000;

    /// Gas attached to the inner callback for processing result of the call to get the current
    /// unstaked balance from the staking pool.
    /// The callback might proceed with withdrawing this amount.
    /// Requires 100e12 for local updates + gas for withdraw + gas for another callback.
    pub const ON_GET_ACCOUNT_UNSTAKED_BALANCE_TO_WITHDRAW: u64 = 100_000_000_000_000
        + crate::gas::staking_pool::WITHDRAW
        + ON_STAKING_POOL_WITHDRAW_FOR_TERMINATION;

    /// Gas attached to the inner callback for processing result of the withdraw call to the
    /// staking pool.
    /// Requires 100e12 for local updates.
    pub const ON_STAKING_POOL_WITHDRAW_FOR_TERMINATION: u64 = 100_000_000_000_000;

    /// Gas attached to the inner callback for processing result of the withdrawal of the
    /// terminated unvested balance.
    /// Requires 100e12 for local updates.
    pub const ON_WITHDRAW_UNVESTED_AMOUNT: u64 = 100_000_000_000_000;
}
