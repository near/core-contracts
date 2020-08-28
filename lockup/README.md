# Lockup / Vesting contract

## Overview

This contract acts as an escrow that locks and holds an owner's tokens for a lockup period. A lockup period starts
from the moment transfers are enabled by voting and lasts for the specified duration.
It's also possible to lock tokens until the absolute timestamp. In this case the tokens will be available after the
lockup period or after the timestamp, whichever is later.

If transfers are not enabled yet, the contract keeps the account ID of the transfer poll contract.
When the transfer poll is resolved, it returns the timestamp when it was resolved and it's used as the beginning of the
lockup period.

Once all tokens are unlocked, everything is vested, and transfers are enabled, the owner can add a full access key to the
account. This will allow the owner to turn this contract account into a regular account, claim the remaining tokens, and remove the contract
or delete the account.

### Vesting schedule

The contract can contain a vesting schedule and serve as a vesting agreement between the NEAR Foundation (Foundation) and an employee (owner of contract).
Vesting schedule is described by three timestamps in nanoseconds:
- `start_timestamp` - When the vesting starts. E.g. the start date of employment.
- `cliff_timestamp` - When the first part of lockup tokens becomes vested.
 The remaining tokens will vest continuously until they are fully vested.
 Example: a 1 year of employment at which moment the 1/4 of tokens become vested.
- `end_timestamp` -  When the vesting ends.

In addition to the lockup period that starts from the moment the transfers are enabled, vesting schedule also locks
all tokens until `cliff_timestamp`.
Once the `cliff_timestamp` passed, the tokens are vested on a pro rata basis from the `start_timestamp` to the `end_timestamp`.

### Release schedule

The release of tokens can also be scheduled to be linear from the moment transfers are enabled.
To achieve this it's possible to specify release duration. Once the transfers are enabled, the release schedule will
start from the timestamp of the vote.
Release schedule can not be terminated by the foundation.

### Staking

NEAR is the proof of stake network. The owner of the lockup contract might hold large percentage of the network tokens.
The owner may want to stake these tokens (including locked tokens) to help secure the network and also earn staking rewards that are distributed to network validator.
This contract doesn't allow to directly stake from this account, so the owner can delegate tokens to a staking pool contract (see https://github.com/near/initial-contracts/tree/master/staking-pool).

The owner can choose the staking pool where to delegate tokens.
The staking pool contract and the account has to be approved and whitelisted by the foundation, to prevent lockup tokens from being lost, locked or stolen.
This staking pool must be an approved account, which is validated by a whitelisting contract.
Once the staking pool holds tokens, the owner of the staking pool is able to use them to vote on the network governance issues, such as enabling transfers.
So it's important for the owner to pick the staking pool that fits the best.

### Early Vesting Termination

In case of vesting schedule, the contract supports the ability for the NEAR Foundation to terminate vesting at any point before it completes.
If the vesting is terminated before the cliff all tokens are refunded to the Foundation. Otherwise the remaining unvested tokens are refunded.

In the event of termination, the vesting stops and the remaining unvested tokens are locked until they are withdrawn by the Foundation.
During termination, the owner can't issue any action towards the staking pool or issue transfers.
If the amount of tokens on the contract account is less than the remaining unvested balance, the Foundation will try to unstake and withdraw everything from the staking pool.
Once the tokens are withdrawn from the staking pool, the Foundation will proceed with withdrawing the unvested balance from the contract.
Once the unvested balance is withdrawn completely, the contract returns to the regular state, and the owner can stake and transfer again.

The amount withdrawn in the event of termination by NEAR foundation may be lower than the initial contract amount.
It's because the contract has to maintain the minimum required balance to cover storage of the contract code and contract state.
The amount of NEAR tokens locked to maintain the minimum storage balance is `35` NEAR.
`35` NEAR is enough to cover storage for `350000` bytes on mainnet at price of `1` NEAR per `10000` bytes.

If there is still a termination balance deficit due to minimum required balance, the owner may decide to fund the deficit on this account to finish the termination process.
This can be done through a regular transfer action from an account with liquid balance.

## Technical details

The contract can be used for the following purposes:
- Lock tokens until the transfers are voted to be enabled.
- Lock tokens for the lockup period and until the absolute timestamp, whichever is later.
- Lock tokens for the lockup period without a vesting schedule. All tokens will be unlocked at once once the lockup period passed.
- Lock tokens for the lockup period with a vesting schedule.
  - If the NEAR Foundation account ID is provided during initialization, the NEAR Foundation can terminate vesting schedule.
  - If the NEAR Foundation account ID is not provided, the vesting schedule can't be terminated.
- Lock tokens for the lockup period with the release duration. Tokens are linearly released on transfers are enabled.

### Guarantees

With the guarantees from the staking pool contracts, whitelist and voting contract, the lockup contract provides the following guarantees:
- The owner can not lose tokens or block contract operations by using methods under staking section.
- The owner can not prevent foundation from withdrawing the unvested balance in case of termination.
- The owner can not withdraw tokens locked due to lockup period, disabled transfers or vesting schedule.
- The owner can withdraw rewards from staking pool, before tokens are unlocked, unless the vesting termination prevents it.
- The owner should be able to add a full access key to the account, once all tokens are vested, unlocked and transfers are enabled.

## Change Log

### `1.0.0`

- Make `release_duration` independent from the `vesting_schedule`. They are not allowed to be used simultaneously.
- Internal. Remove some JSON serialization on inner structures.
- Fix a bug with the prepaid gas exceeded during the foundation callback by increasing base gas.
- Include minimum amount of gas needed for every call.
- Add new helper methods for the owner for staking.
    - `deposit_and_stake`, `unstake_all`, `withdraw_all_from_staking_pool`
- Add a new view method to `get_balance` of the account, that includes all tokens on this account and all tokens deposited to a staking pool.
- Cover foundation termination flow with the integration tests.
- Cover release schedule flow with integration tests.
- Updated `near-sdk` to `2.0.0`

### `0.3.0`

- Introduced optional release duration
- Introduced optional absolute lockup timestamp.
- Updated `init` arguments
    - Added optional absolute `lockup_timestamp`.
    - Renamed `lockup_start_information` to `transfers_information`. Also renamed internal timestamp to `transfers_timestamp`.
    - Added optional `release_duration` to linearly release tokens.

### `0.2.0`

- Replaced owner's access keys with the owner's account. The access is now controlled through the predecessor account ID similar to NEAR foundation access.
  This allows to be more flexible with the account access including multi-sig implementation.
- The lockup contract account should not have any access keys until the account is fully vested and unlocked.
  Only then the owner can add the full access key.
- Removed methods for adding and removing staking/main access keys.
- Added a view method to get the account ID of the owner.

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
/// Requires 25 TGas (1 * BASE_GAS)
///
/// Initializes lockup contract.
/// - `owner_account_id` - the account ID of the owner.  Only this account can call owner's
///    methods on this contract.
/// - `lockup_duration` - the duration in nanoseconds of the lockup period from the moment
///    the transfers are enabled.
/// - `lockup_timestamp` - the optional absolute lockup timestamp in nanoseconds which locks
///    the tokens until this timestamp passes.
/// - `transfers_information` - the information about the transfers. Either transfers are
///    already enabled, then it contains the timestamp when they were enabled. Or the transfers
///    are currently disabled and it contains the account ID of the transfer poll contract.
/// - `vesting_schedule` - if present, describes the vesting schedule for employees. Vesting
///    schedule affects the amount of tokens the NEAR Foundation will get in case of
///    employment termination as well as the amount of tokens available for transfer by
///    the employee.
/// - `release_duration` - is the duration when the full lockup amount will be available.
///    The tokens are linearly released from the moment transfers are enabled. If it's used
///    in addition to the vesting schedule, then the amount of tokens available to transfer
///    is subject to the minimum between vested tokens and released tokens.
/// - `staking_pool_whitelist_account_id` - the Account ID of the staking pool whitelist contract.
/// - `foundation_account_id` - the account ID of the NEAR Foundation, that has the ability to
///    terminate vesting schedule.
#[init]
pub fn new(
    owner_account_id: AccountId,
    lockup_duration: WrappedDuration,
    lockup_timestamp: Option<WrappedTimestamp>,
    transfers_information: TransfersInformation,
    vesting_schedule: Option<VestingSchedule>,
    release_duration: Option<WrappedDuration>,
    staking_pool_whitelist_account_id: AccountId,
    foundation_account_id: Option<AccountId>,
) -> Self;
```

It requires to provide `LockupStartInformation` and `VestingSchedule`

```rust
/// Contains information about the transfers. Whether transfers are enabled or disabled.
pub enum TransfersInformation {
    /// The timestamp when the transfers were enabled. The lockup period starts at this timestamp.
    TransfersEnabled {
        transfers_timestamp: WrappedTimestamp,
    },
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

Owner's methods are split into 2 groups. Methods that are related to transfers and methods that are related to staking.
It's safer to give access to the staking methods, because they can't lose or lock tokens. It should be possible to
create an access key on the owner's account restricted to the staking methods and give it to staking pool manager UI. e.g. Staking Pool UI in the browser.

#### Transfers methods

```rust
/// OWNER'S METHOD
///
/// Requires 75 TGas (3 * BASE_GAS)
///
/// Calls voting contract to validate if the transfers were enabled by voting. Once transfers
/// are enabled, they can't be disabled anymore.
pub fn check_transfers_vote(&mut self) -> bool;

/// OWNER'S METHOD
///
/// Requires 50 TGas (2 * BASE_GAS)
///
/// Transfers the given amount to the given receiver account ID.
/// This requires transfers to be enabled within the voting contract.
pub fn transfer(&mut self, amount: WrappedBalance, receiver_id: AccountId);

/// OWNER'S METHOD
///
/// Requires 50 TGas (2 * BASE_GAS)
///
/// Adds full access key with the given public key to the account once the contract is fully
/// vested, lockup duration has expired and transfers are enabled.
/// This will allow owner to use this account as a regular account and remove the contract.
pub fn add_full_access_key(&mut self, new_public_key: Base58PublicKey);
```

#### Staking methods

```rust
/// OWNER'S METHOD
///
/// Requires 75 TGas (3 * BASE_GAS)
///
/// Selects staking pool contract at the given account ID. The staking pool first has to be
/// checked against the staking pool whitelist contract.
pub fn select_staking_pool(&mut self, staking_pool_account_id: AccountId) -> bool;

/// OWNER'S METHOD
///
/// Requires 25 TGas (1 * BASE_GAS)
///
/// Unselects the current staking pool.
/// It requires that there are no known deposits left on the currently selected staking pool.
pub fn unselect_staking_pool(&mut self);

/// OWNER'S METHOD
///
/// Requires 100 TGas (4 * BASE_GAS)
///
/// Deposits the given extra amount to the staking pool
pub fn deposit_to_staking_pool(&mut self, amount: WrappedBalance) -> bool;

/// OWNER'S METHOD
///
/// Requires 125 TGas (5 * BASE_GAS)
///
/// Deposits and stakes the given extra amount to the selected staking pool
pub fn deposit_and_stake(&mut self, amount: WrappedBalance) -> bool;

/// OWNER'S METHOD
///
/// Requires 125 TGas (5 * BASE_GAS)
///
/// Withdraws the given amount from the staking pool
pub fn withdraw_from_staking_pool(&mut self, amount: WrappedBalance) -> bool;

/// OWNER'S METHOD
///
/// Requires 175 TGas (7 * BASE_GAS)
///
/// Tries to withdraws all unstaked balance from the staking pool.
pub fn withdraw_all_from_staking_pool(&mut self) -> bool;

/// OWNER'S METHOD
///
/// Requires 125 TGas (5 * BASE_GAS)
///
/// Stakes the given extra amount at the staking pool
pub fn stake(&mut self, amount: WrappedBalance) -> bool;

/// OWNER'S METHOD
///
/// Requires 125 TGas (5 * BASE_GAS)
///
/// Unstakes the given amount at the staking pool
pub fn unstake(&mut self, amount: WrappedBalance) -> bool;

/// OWNER'S METHOD
///
/// Requires 125 TGas (5 * BASE_GAS)
///
/// Unstakes all tokens at the staking pool
pub fn unstake_all(&mut self) -> bool;

/// OWNER'S METHOD
///
/// Requires 75 TGas (3 * BASE_GAS)
///
/// Retrieves total balance from the staking pool and remembers it internally.
/// This method is helpful when the owner received some rewards for staking and wants to
/// transfer them back to this account for withdrawal. In order to know the actual liquid
/// balance on the account, this contract needs to query the staking pool.
pub fn refresh_staking_pool_balance(&mut self);
```

### Foundation methods

In case of employee vesting, the NEAR Foundation will be able to call them from the foundation account and be able to
terminate vesting schedule.

```rust
/// FOUNDATION'S METHOD
///
/// Requires 25 TGas (1 * BASE_GAS)
///
/// Terminates vesting schedule and locks the remaining unvested amount.
pub fn terminate_vesting(&mut self);

/// FOUNDATION'S METHOD
///
/// Requires 175 TGas (7 * BASE_GAS)
///
/// When the vesting is terminated and there are deficit of the tokens on the account, the
/// deficit amount of tokens has to be unstaked and withdrawn from the staking pool.
pub fn termination_prepare_to_withdraw(&mut self) -> bool;

/// FOUNDATION'S METHOD
///
/// Requires 75 TGas (3 * BASE_GAS)
///
/// Withdraws the unvested amount from the early termination of the vesting schedule.
pub fn termination_withdraw(&mut self, receiver_id: AccountId) -> bool;
```

### View methods

```rust
/// Returns the account ID of the owner.
pub fn get_owner_account_id(&self) -> AccountId;

/// Returns the account ID of the selected staking pool.
pub fn get_staking_pool_account_id(&self) -> Option<AccountId>;

/// The amount of tokens that were deposited to the staking pool.
/// NOTE: The actual balance can be larger than this known deposit balance due to staking
/// rewards acquired on the staking pool.
/// To refresh the amount the owner can call `refresh_staking_pool_balance`.
pub fn get_known_deposited_balance(&self) -> WrappedBalance;

/// Returns the current termination status or `None` in case of no termination.
pub fn get_termination_status(&self) -> Option<TerminationStatus>;

/// The amount of tokens that are not going to be vested, because the vesting schedule was
/// terminated earlier.
pub fn get_terminated_unvested_balance(&self) -> WrappedBalance;

/// The amount of tokens missing from the account balance that are required to cover
/// the unvested balance from the early-terminated vesting schedule.
pub fn get_terminated_unvested_balance_deficit(&self) -> WrappedBalance;

/// Get the amount of tokens that are already vested or released, but still locked due to lockup.
pub fn get_locked_amount(&self) -> WrappedBalance;

/// Get the amount of tokens that are already vested, but still locked due to lockup.
pub fn get_locked_vested_amount(&self) -> WrappedBalance;

/// Get the amount of tokens that are locked in this account due to vesting or release schedule.
pub fn get_unvested_amount(&self) -> WrappedBalance;

/// The balance of the account owner. It includes vested and extra tokens that may have been
/// deposited to this account, but excludes locked tokens.
/// NOTE: Some of this tokens may be deposited to the staking pool.
/// This method also doesn't account for tokens locked for the contract storage.
pub fn get_owners_balance(&self) -> WrappedBalance;

/// Returns total balance of the account including tokens deposited on the staking pool.
pub fn get_balance(&self) -> WrappedBalance;

/// The amount of tokens the owner can transfer now from the account.
/// Transfers have to be enabled.
pub fn get_liquid_owners_balance(&self) -> WrappedBalance;

/// Returns `true` if transfers are enabled, `false` otherwise.
pub fn are_transfers_enabled(&self) -> bool;
```

## API examples

### Initialization

Initialize contract, assuming it's called from `near` account.
The lockup contract account ID is `lockup1`.
The owner account ID is `owner1`. Lockup Duration is 365 days.
Transfers are currently disabled and can be enabled by checking transfer voting poll contract at `transfers-poll`.
Vesting is 4 years starting from `2018-09-01` to `2022-09-01` Pacific time.
Staking pool whitelist contract is at `staking-pool-whitelist`.
The foundation account ID that can terminate vesting is `near`.

Arguments in JSON format

```json
{
    "owner_account_id": "owner1",
    "lockup_duration": "31536000000000000",
    "transfers_information": {
        "TransfersDisabled": {
            "transfer_poll_account_id": "transfers-poll"
        }
    },
    "vesting_schedule": {
        "start_timestamp": "1535760000000000000",
        "cliff_timestamp": "1567296000000000000",
        "end_timestamp": "1661990400000000000"
    },
    "staking_pool_whitelist_account_id": "staking-pool-whitelist",
    "foundation_account_id": "near"
}
```

Command

```bash
near call lockup1 new '{"owner_account_id": "owner1", "lockup_duration": "31536000000000000", "transfers_information": {"TransfersDisabled": {"transfer_poll_account_id": "transfers-poll"}}, "vesting_schedule": {"start_timestamp": "1535760000000000000", "cliff_timestamp": "1567296000000000000", "end_timestamp": "1661990400000000000"}, "staking_pool_whitelist_account_id": "staking-pool-whitelist", "foundation_account_id": "near"}' --accountId=near --gas=25000000000000
```

#### Other examples of initialization

##### Adding lockup timestamp with relative lockup period of 14 days (whichever is later).

```json
{
    "owner_account_id": "owner1",
    "lockup_duration": "1209600000000000",
    "lockup_timestamp": "1661990400000000000",
    "transfers_information": {
        "TransfersDisabled": {
            "transfer_poll_account_id": "transfers-poll"
        }
    },
    "staking_pool_whitelist_account_id": "staking-pool-whitelist",
}
```

##### With release duration for 2 years as linear release and 14 days lockup period.

```json
{
    "owner_account_id": "owner1",
    "lockup_duration": "1209600000000000",
    "transfers_information": {
        "TransfersDisabled": {
            "transfer_poll_account_id": "transfers-poll"
        }
    },
    "release_duration": "63072000000000000",
    "staking_pool_whitelist_account_id": "staking-pool-whitelist",
}
```

### Staking flow

#### Select staking pool

```bash
near call lockup1 select_staking_pool '{"staking_pool_account_id": "staking_pool_pro"}' --accountId=owner1 --gas=75000000000000
```

#### Deposit and stake to the staking pool

Deposit and stake `1000` NEAR tokens.

```bash
near call lockup1 deposit_and_stake '{"amount": "1000000000000000000000000000"}' --accountId=owner1 --gas=125000000000000
```

#### Refresh the current total balance on the staking pool

When the owner has accumulated some rewards on the staking pool, the contract doesn't let the owner to withdraw them yet.
It's because the contract doesn't know about the accumulated rewards.
In order for the contract to get the new total balance, the owner has to call `refresh_staking_pool_balance`.

```bash
near call lockup1 refresh_staking_pool_balance '{}' --accountId=owner1 --gas=75000000000000
```

#### Checking owner's balance

If the owner has accumulated 10 NEAR in the rewards, after refreshing the staking pool balance, the owner should see
the local balance to increase as well.

```bash
near view lockup1 get_owners_balance '{}'
```

#### Unstake from the staking pool

Let's say the owner checked staked balance by calling view method on the staking pool directly and decided to unstake everything.

```bash
near call lockup1 unstake_all '{}' --accountId=owner1 --gas=125000000000000
```

#### Withdraw from the staking pool

Wait 4 epochs (about 48 hours) and now can withdraw all NEAR tokens from the staking pool.

```bash
near call lockup1 withdraw_all_from_staking_pool '{}' --accountId=owner1 --gas=175000000000000
```

#### Check transfers vote

```bash
near call lockup1 check_transfers_vote '{}' --accountId=owner1 --gas=75000000000000
```

Let's assume transfers are enabled now.


#### Check liquid balance and transfer 10 NEAR

```bash
near view lockup1 get_liquid_owners_balance '{}'
```

Transfer 10 NEAR to `owner-sub-account`.

```bash
near call lockup1 transfer '{"amount": "10000000000000000000000000", "receiver_id": "owner-sub-account"}' --accountId=owner1 --gas=50000000000000
```

#### Adding full access key

Once everything is unlocked the owner can add a full access key on the lockup account. This allows to withdraw remaining tokens locked due to contract storage.
The owner first should generate a new key-pair (private and public keys). Then the owner should pass the public key from this key-pair.

```bash
near call lockup1 add_full_access_key '{"new_public_key": "CE3QAXyVLeScmY9YeEyR3Tw9yXfjBPzFLzroTranYtVb"}' --accountId=owner1 --gas=50000000000000
```

Now owner should be able to delete this account and claim all tokens.
WARNING: This should only be done if there is no tokens delegated to a staking pool. Otherwise those tokens might be lost.

This command with delete `lockup1` and transfer all tokens remaining tokens from the lockup account to `owner1`

```bash
near delete lockup1 owner1
```


### Vesting termination by NEAR Foundation

If the employee was terminated, the foundation needs to terminate vesting.

#### Initiate termination

To initiate termination NEAR Foundation has to issue the following command:

```bash
near call lockup1 terminate_vesting '' --accountId=near --gas=25000000000000
```

This will block the account until the termination process is completed.

#### Monitoring status

To check the current status of the termination process, the foundation and the owner can call:

```bash
near view lockup1 get_termination_status '{}'
```

#### Withdrawing deficit from the staking pool

If the owner staked with some staking pool and the unvested amount is larger than the current liquid balance, then
it creates the deficit (otherwise the foundation can proceed with withdrawal).

The current termination status should be `VestingTerminatedWithDeficit`.

The NEAR Foundation needs to first unstake tokens in the staking pool and then once tokens
become liquid, withdraw them from the staking pool to the contract. This is done by calling `termination_prepare_to_withdraw`.

```bash
near call lockup1 termination_prepare_to_withdraw '{}' --accountId=near --gas=175000000000000
```

The first will unstake everything from the staking pool. This should advance the termination status to `EverythingUnstaked`.
In 4 epochs, or about 48 hours, the Foundation can call the same command again:

```bash
near call lockup1 termination_prepare_to_withdraw '{}' --accountId=near --gas=175000000000000
```

If everything went okay, the status should be advanced to `ReadyToWithdraw`.

### Withdrawing from the account

Once the termination status is `ReadyToWithdraw`. The Foundation can proceed with withdrawing the unvested balance.

```bash
near call lockup1 termination_withdraw '{"receiver_id": "near"}' --accountId=near --gas=75000000000000
```

In case of successful withdrawal, the unvested balance will become `0` and the owner can use this contract again.
