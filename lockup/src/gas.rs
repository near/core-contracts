pub mod whitelist {
    /// Gas attached to the promise to check whether the given staking pool Account ID is
    /// whitelisted.
    /// Requires 100e12 (no external calls).
    pub const IS_WHITELISTED: u64 = 100_000_000_000_000;
}

pub mod staking_pool {
    /// The amount of gas required for a voting through a staking pool.
    /// Requires 100e12 for execution + 200e12 for attaching to a call on the voting contract.
    pub const VOTE: u64 = 300_000_000_000_000;

    /// The amount of gas required to get total user balance from the staking pool.
    /// Requires 100e12 for local processing.
    pub const GET_TOTAL_USER_BALANCE: u64 = 100_000_000_000_000;

    /// Gas attached to deposit call on the staking pool contract.
    /// Requires 100e12 for local updates.
    pub const DEPOSIT: u64 = 100_000_000_000_000;

    /// Gas attached to withdraw call on the staking pool contract.
    /// Requires 100e12 for execution + 200e12 for transferring amount to us.
    pub const WITHDRAW: u64 = 300_000_000_000_000;

    /// Gas attached to stake call on the staking pool contract.
    /// Requires 100e12 for execution + 200e12 for staking call.
    pub const STAKE: u64 = 300_000_000_000_000;

    /// Gas attached to unstake call on the staking pool contract.
    /// Requires 100e12 for execution + 200e12 for staking call.
    pub const UNSTAKE: u64 = 300_000_000_000_000;
}

pub mod voting {
    /// Gas attached to the promise to check whether transfers were enabled on the voting
    /// contract.
    /// Requires 100e12 (no external calls).
    pub const GET_RESULT: u64 = 100_000_000_000_000;
}

pub mod callbacks {
    /// Gas attached to the inner callback for processing whitelist check results.
    /// Requires 100e12 for local execution.
    pub const ON_WHITELIST_IS_WHITELISTED: u64 = 100_000_000_000_000;

    /// Gas attached to the inner callback for processing result of the call to get balance on
    /// the staking pool balance.
    /// Requires 100e12 for local updates.
    pub const ON_STAKING_POOL_GET_TOTAL_USER_BALANCE: u64 = 100_000_000_000_000;

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

    /// Gas attached to the inner callback for processing result of the withdrawal of the
    /// terminated unvested balance.
    /// Requires 100e12 for local updates.
    pub const ON_WITHDRAW_UNVESTED_AMOUNT: u64 = 100_000_000_000_000;
}
