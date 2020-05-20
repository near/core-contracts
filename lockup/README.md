# Lockup / Vesting contract

## Overview

This contract acts as an escrow that locks and holds owner's funds for the lockup period. The lockup period either starts
at the given timestamp or from the moment transfers are enabled by voting.
If transfers are not enabled yet, the contract keeps the account ID of the transfer poll contract.
When the transfer poll is resolved, it returns the timestamp when it was resolved and it's used as the beginning of the
lockup period.

Once all funds are unlocked (including vesting) and transfers are enabled, the owner can add a full access key to the
account. This will allow the owner to turn this account into a regular account, remove the contract, or delete the account
and claim the remaining funds.

### Vesting

The contract can also contain a vesting schedule.
In this case, this contract serves as a vesting agreement between the NEAR Foundation (Foundation) and an employee (owner of contract).
Vesting schedule is described by 3 timestamps in nanoseconds:
- `start_timestamp` - The timestamp in nanosecond when the vesting starts. E.g. the start date of employment.
- `cliff_timestamp` - The timestamp in nanosecond when the first part of lockup tokens becomes vested.
 The remaining tokens will vest continuously until they are fully vested.
 Example: a 1 year of employment at which moment the 1/4 of tokens become vested.
- `end_timestamp` - The timestamp in nanosecond when the vesting ends.

In addition to the lockup period that starts from the moment the transfers are enabled, vesting schedule also locks
all funds until `cliff_timestamp`.
Once the `cliff_timestamp` passed, the funds are vested linearly from the `start_timestamp` to the `end_timestamp`.

### Staking

NEAR is the proof of stake network. The owner of the lockup contract might hold large percentage of the network tokens.
The owner may want to stake these tokens (including locked tokens) to help secure the network and also earn staking rewards that are distributed to network validator.
The contract doesn't allow to directly stake from this account, so the owner can delegate tokens to a staking pool contract (see https://github.com/near/initial-contracts/tree/master/staking-pool).

The owner can choose the staking pool where to delegate tokens.
The staking pool contract and the account has to be approved and whitelisted by the foundation, to prevent lockup tokens from being lost, locked or stolen.
This staking pool must be an approved account, which is validated by a whitelisting contract.
Once the staking pool holds tokens, the owner of the staking pool is able to use them to vote on the network governence issues, such as enabling transfers.
So it's important for the owner to pick the staking pool that fits the best.

### Early Vesting Termination

In case of vesting schedule, the contract supports the ability for the NEAR Foundation to terminate vesting at any point before it completes.
If the vesting is terminated before the cliff all tokens are refunded to the Foundation. Otherwise the remaining unvested funds are refunded.

In the event of termination, the vesting stops and the remaining unvested funds are locked until they are withdrawn by the Foundation.
During termination, the owner can't issue any action towards the staking pool or issue transfers.
If the amount of tokens on the contract account is less than the remaining unvested balance, the Foundation will try to unstake and withdraw everything from the staking pool.
Once the funds are withdrawn from the staking pool, the Foundation will proceed with withdrawing the unvested balance from the contract.
Once the unvested balance is withdrawn completely, the contract returns to the regular state, and the owner can stake and transfer again.

## Technical details

The contract can be used for the following purposes:
- Lock funds for the lockup period without vesting schedule. All funds will be unlocked at once once the lockup period passed.
- Lock funds for the lockup period with vesting schedule.
  - If the foundation access key is given during initialization, the NEAR Foundation can terminate vesting schedule.



Formally the contract includes:

1. Lockup information.
2. A whitelisted staking pool's account ID.
3. A transfer poll account ID.
4. Employee (Owner)'s public key and optionally their staking public key.
5. Optionally the public key of the Foundation.


### Lockup Information
This includes:
1. Amount of tokens to lockup.
2. Length of lockup.
3. Optional timestamp of when transfers started.
4. Optional vesting information

#### Vesting Information
Either:
1. Vesting Schedule:
  A) Start timestamp time in nano-seconds.
  B) Cliff timestamp
  C) End timestamp

2. Termination Status:
  A)
  B)




## Interface

### Basic types

The contract uses basic types that are wrapped into structures to support JSON serialization and deserialization towards strings for long integers.

```rust
/// Timestamp in nanosecond wrapped into a struct for JSON serialization as a string.
pub type WrappedTimestamp = U64;
/// Duration in nanosecond wrapped into a struct for JSON serialization as a string.
pub type WrappedDuration = U64;
/// Balance wrapped into a struct for JSON serialization as a string.
pub type WrappedBalance = U128;
```

### Initialization

The initialization method has the following interface.

```rust
/// Initializes lockup contract.
/// - `lockup_duration` - the duration in nanoseconds of the lockup period.
/// - `lockup_start_information` - the information when the lockup period starts, either
///    transfers are already enabled, then it contains the timestamp, or the transfers are
///    currently disabled and it contains the account ID of the transfer poll contract.
/// - `vesting_schedule` - if present, describes the vesting schedule.
/// - `staking_pool_whitelist_account_id` - the Account ID of the staking pool whitelist contract.
/// - `owners_main_public_key` - the public key for the owner's main access key.
/// - `owners_staking_public_key` - the public key for the owner's access key for staking
///    pool operations (optional).
/// - `foundation_public_key` - the public key for NEAR foundation's access key to be able to
///    terminate vesting schedule.
#[init]
pub fn new(
    lockup_duration: WrappedDuration,
    lockup_start_information: LockupStartInformation,
    vesting_schedule: Option<VestingSchedule>,
    staking_pool_whitelist_account_id: AccountId,
    owners_main_public_key: Base58PublicKey,
    owners_staking_public_key: Option<Base58PublicKey>,
    foundation_public_key: Option<Base58PublicKey>,
) -> Self;
```

It requires to provide `LockupStartInformation` and `VestingSchedule`

```rust
/// Contains information when the lockup period starts.
pub enum LockupStartInformation {
    /// The timestamp when the transfers were enabled. The lockup period starts at this timestamp.
    TransfersEnabled { lockup_timestamp: WrappedTimestamp },
    /// The account ID of the transfers poll contract, to check if the transfers are enabled.
    /// The lockup period will start when the transfer voted to be enabled.
    /// At the launch of the network transfers are disabled for all lockup contracts, once transfers
    /// are enabled, they can't be disabled and don't need to be checked again.
    TransfersDisabled { transfer_poll_account_id: AccountId },
}

/// Contains information about vesting schedule.
pub struct VestingSchedule {
    /// The timestamp in nanosecond when the vesting starts. E.g. the start date of employment.
    pub start_timestamp: WrappedTimestamp,
    /// The timestamp in nanosecond when the first part of lockup tokens becomes vested.
    /// The remaining tokens will vest continuously until they are fully vested.
    /// Example: a 1 year of employment at which moment the 1/4 of tokens become vested.
    pub cliff_timestamp: WrappedTimestamp,
    /// The timestamp in nanosecond when the vesting ends.
    pub end_timestamp: WrappedTimestamp,
}
```

### Owner's methods

Owner's methods are split into 2 groups. Methods that can be called by the main access key and methods that can be called by the staking access keys.
The staking access key can be changed with the main access key, so it's okay to store it in the less secure location, e.g. Staking Pool UI in the browser.
On the other hand, the main access key has to be kept secret and secure, because it can't be changed and if it's lost or compromised, the owner can't recover the balance.

#### Main Access Key methods

```rust
/// OWNER'S METHOD
/// Calls voting contract to validate if the transfers were enabled by voting. Once transfers
/// are enabled, they can't be disabled anymore.
pub fn check_transfers_vote(&mut self) -> Promise;

/// OWNER'S METHOD
/// Transfers the given extra amount to the given receiver account ID.
/// This requires transfers to be enabled within the voting contract.
pub fn transfer(&mut self, amount: WrappedBalance, receiver_id: AccountId) -> Promise;

/// OWNER'S METHOD
/// Changes owner's staking access key to the new given public key.
pub fn change_staking_access_key(&mut self, new_public_key: Base58PublicKey) -> Promise;

/// OWNER'S METHOD
/// Adds full access key with the given public key to the account once the contract is fully
/// vested, lockup duration has expired and transfers are enabled.
/// This will allow owner to use this account as a regular account and remove the contract.
pub fn add_full_access_key(&mut self, new_public_key: Base58PublicKey) -> Promise;
```

#### Staking Access Key methods

```rust
/// OWNER'S METHOD
/// Selects staking pool contract at the given account ID. The staking pool first has to be
/// checked against the staking pool whitelist contract.
pub fn select_staking_pool(&mut self, staking_pool_account_id: AccountId) -> Promise;

/// OWNER'S METHOD
/// Unselects the current staking pool.
/// It requires that there are no known deposits left on the currently selected staking pool.
pub fn unselect_staking_pool(&mut self);

/// OWNER'S METHOD
/// Deposits the given extra amount to the staking pool
pub fn deposit_to_staking_pool(&mut self, amount: WrappedBalance) -> Promise;

/// OWNER'S METHOD
/// Withdraws the given amount from the staking pool
pub fn withdraw_from_staking_pool(&mut self, amount: WrappedBalance) -> Promise;

/// OWNER'S METHOD
/// Stakes the given extra amount at the staking pool
pub fn stake(&mut self, amount: WrappedBalance) -> Promise;

/// OWNER'S METHOD
/// Unstakes the given amount at the staking pool
pub fn unstake(&mut self, amount: WrappedBalance) -> Promise;
```

### Foundation methods

In case of employee vesting, the NEAR Foundation controls the access key towards the termination methods.

```rust
/// FOUNDATION'S METHOD
/// Terminates vesting schedule and locks the remaining unvested amount.
pub fn terminate_vesting(&mut self);

/// FOUNDATION'S METHOD
/// When the vesting is terminated and there are deficit of the tokens on the account, the
/// deficit amount of tokens has to be unstaked and withdrawn from the staking pool.
pub fn termination_prepare_to_withdraw(&mut self) -> Promise;

/// FOUNDATION'S METHOD
/// Withdraws the unvested amount from the early termination of the vesting schedule.
pub fn termination_withdraw(&mut self, receiver_id: AccountId) -> Promise;
```

### View methods

```rust
/// Returns the account ID of the selected staking pool.
pub fn get_staking_pool_account_id(&self) -> Option<AccountId>;

/// The amount of tokens that were deposited to the staking pool.
/// NOTE: The actual balance can be larger than this known deposit balance due to staking
/// rewards acquired on the staking pool.
pub fn get_known_deposited_balance(&self) -> WrappedBalance;

/// Returns the current termination status or `None` in case of no termination.
pub fn get_termination_status(&self) -> Option<TerminationStatus>;

/// The amount of tokens that are not going to be vested, because the vesting schedule was
/// terminated earlier.
pub fn get_terminated_unvested_balance(&self) -> WrappedBalance;

/// The amount of tokens missing from the account balance that are required to cover
/// the unvested balance from the early-terminated vesting schedule.
pub fn get_terminated_unvested_balance_deficit(&self) -> WrappedBalance;

/// Get the amount of tokens that are locked in this account due to lockup or vesting.
pub fn get_locked_amount(&self) -> WrappedBalance;

/// Get the amount of tokens that are locked in this account due to vesting.
pub fn get_unvested_amount(&self) -> WrappedBalance;

/// The balance of the account owner. It includes vested and extra tokens that may have been
/// deposited to this account.
/// NOTE: Some of this tokens may be deposited to the staking pool.
/// This method also doesn't account for tokens locked for the contract storage.
pub fn get_owners_balance(&self) -> WrappedBalance;

/// The amount of tokens the owner can transfer now from the account.
pub fn get_liquid_owners_balance(&self) -> WrappedBalance;

/// Returns `true` if transfers are enabled, `false` otherwise.
pub fn are_transfers_enabled(&self) -> bool;
```
