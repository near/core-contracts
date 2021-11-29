# Lockup / Vesting contract

## Overview

This contract acts as an escrow that locks and holds an owner's tokens for a lockup period.
The contract consists of lockup and vesting processes that go simultaneously.
A high-level overview could be found [in NEAR documentation](https://docs.near.org/docs/tokens/lockup).

A lockup period starts from the specified timestamp and lasts for the specified duration.
Tokens will be unlocked linearly.

Vesting is an additional mechanism. It also locks the tokens, and it allows to configure 2 more options:
1. Ability to terminate tokens vesting and refund non-vested tokens back.
2. Cliff vesting period.

The owner can add a full access key to the account if all conditions are met:
- No transaction is in progress;
- Vesting and lockup processes finished;
- The termination process is ended (if applicable). If there’s a termination process started, it has to finish;
- [Transfers are enabled](https://near.org/blog/near-mainnet-is-now-community-operated/) ([Phase II launch on MainNet enabled transfers](https://near.org/blog/near-mainnet-phase-2-unrestricted-decentralized/)).

This will allow the owner to turn this contract account into a regular account, claim the remaining tokens, and remove the contract or delete the account.

### Lockup schedule

Lockup is a mechanism of linear unlocking of tokens that could not be terminated.
It is described by the following fields:
- `lockup_timestamp` - The moment when tokens start linearly unlocking;
- `lockup_duration` - [deprecated] Alternative way to set the moment when the tokens become unlock.
  The duration from [the moment transfers were enabled](https://near.org/blog/near-mainnet-phase-2-unrestricted-decentralized/) to the moment when linear unlocking begins;
- `release_duration` - The length of the unlocking schedule during which tokens are linearly unlocked.
  By the end of this duration all tokens are unlocked.
  `finish_timestamp = lockup_timestamp + release_duration`.

If `lockup_timestamp` and `lockup_duration` are not specified, the lockup starts from the timestamp from [`transfers_information`](https://github.com/near/core-contracts/blob/master/lockup/src/lib.rs#L187) field.
It's usually the moment when [transfers were enabled by voting](https://near.org/blog/near-mainnet-phase-2-unrestricted-decentralized/) in the system: 2020-10-13, 18:38:58 UTC or `1602614338293769340` nanoseconds unix time.

### Vesting schedule

The contract can contain a vesting schedule and serve as a vesting agreement between the foundation and an employee (owner of the contract).
The foundation is set at the moment of initializing the contract by the `foundation_account_id` field.

A vesting schedule is described by three timestamps in nanoseconds:
- `start_timestamp` - When the vesting starts. E.g. the start date of employment;
- `cliff_timestamp` - When the first part of lockup tokens becomes vested.
  The remaining tokens will vest continuously until they are fully vested.
  Assume we have a 4-year contract with a 1-year cliff.
  In the first year, nothing is vested, then 25% is vested, then we have linear vesting till the end of the contract.
  25% is the number calculated by the formula:
  ```
  cliff_tokens_percentage = (cliff_timestamp - start_timestamp) / (end_timestamp - start_timestamp)
  ```
- `end_timestamp` -  When the vesting ends.

Once the `cliff_timestamp` passed, the tokens are vested on a pro-rata basis from the `start_timestamp` to the `end_timestamp`.

### Combining lockup and vesting

The contract could have both lockup and vesting schedules.

The tokens start to become liquid at the timestamp:
```
liquidity_timestamp = max(max(transfers_enabled_timestamp + lockup_duration, lockup_timestamp), cliff_timestamp)
```

The current amount of non-liquid tokens are calculated as the maximum between lockup and vesting logic.
If at least one mechanism said the tokens are locked, then they are still locked.

The contract could also have only one of these mechanisms.
When initializing, it's possible to pass empty vesting information, then we use a lockup schedule.
It's also possible not to provide `release_duration`, it means that we use a vesting schedule.
If neither of the mechanisms is initialized, the tokens will become liquid after transfers enabled moment ([`transfers_information`](https://github.com/near/core-contracts/blob/master/lockup/src/lib.rs#L187) field).

### Staking

NEAR is the proof of stake network. The owner of the lockup contract might hold a large percentage of the network tokens.
The owner may want to stake these tokens (including locked/unvested tokens) to help secure the network and also earn staking rewards that are distributed to the network validator.
This contract doesn't allow to directly stake from this account, so the owner can delegate tokens to a [staking pool contract](https://github.com/near/initial-contracts/tree/master/staking-pool).

The owner can choose the staking pool for delegating tokens.
The staking pool contract and the account have to be approved by the whitelisting contract to prevent tokens from being lost, locked, or stolen.
Whitelisting contract is set at the moment of initializing the Lockup contract by [`staking_pool_whitelist_account_id`](https://github.com/near/core-contracts/blob/master/lockup/src/lib.rs#L190) field.
Once the staking pool holds tokens, the owner of the staking pool can use them to vote on the network governance issues, such as enabling transfers.
So the owner needs to pick the staking pool that fits the best.

### Early Vesting Termination

In the case of the vesting schedule, the contract supports the ability for the foundation to terminate vesting at any point before it completes.
If the vesting is terminated before the cliff all tokens are refunded to the foundation. Otherwise, the remaining unvested tokens are refunded.

In the event of termination, the vesting stops, and the remaining unvested tokens are locked until they are withdrawn by the foundation.
During termination, the owner can't issue any action towards the staking pool or issue transfers.
If the amount of tokens on the contract account is less than the remaining unvested balance, the foundation will try to unstake and withdraw everything from the staking pool.
Once the tokens are withdrawn from the staking pool, the foundation will proceed with withdrawing the unvested balance from the contract.
Once the unvested balance is withdrawn completely, the contract returns to the regular state, and the owner can stake and transfer again.

The amount withdrawn in the event of termination by the foundation may be lower than the initial contract amount.
It's because the contract has to maintain the minimum required balance to cover storage of the contract code and contract state.

### Guarantees

With the guarantees from the staking pool contracts, whitelist, and voting contract, the lockup contract provides the following guarantees:
- The owner can not lose tokens or block contract operations by using methods under the staking section.
- The owner can not prevent the foundation from withdrawing the unvested balance in case of termination.
- The owner can not withdraw tokens locked due to lockup period, disabled transfers, or vesting schedule.
- The owner can withdraw rewards from the staking pool before tokens are unlocked unless the vesting termination prevents it.
- The owner should be able to add a full access key to the account, once all tokens are vested, unlocked and transfers are enabled.

### Contributing

We use Docker to build the contract.
Configuration could be found [here](https://github.com/near/near-sdk-rs/tree/master/contract-builder).
Please make sure that Docker is given at least 4Gb of RAM.

### [Deprecated] Private vesting schedule

Since the vesting schedule usually starts at the date of employment it allows to de-anonymize the owner of the lockup contract.
To keep the identity private, the contract allows to hash the vesting schedule with some random salt and keep store the hash instead of the raw vesting schedule information.
In case the foundation has to terminate the vesting schedule, it will provide the raw vesting schedule and the salt, effectively revealing the vesting schedule.
The contract then will compare the hash with the internal hash and if they match proceed with the termination.

**NOTE**: The private vesting schedule can only be used if the lockup release period and the lockup duration are effectively shadowing the vesting duration.
Meaning that the lockup release ends later than the vesting release and the lockup duration ends after the vesting cliff.
Once the lockup schedule starts before the vesting schedule (e.g. employment starts after the transfers are enabled), the vesting schedule can't be kept private anymore.

## Interface

Here are some useful links to the documented codebase:
- [The initialization method](https://github.com/near/core-contracts/blob/master/lockup/src/lib.rs#L151-L190);
- [Basic types](https://github.com/near/core-contracts/blob/master/lockup/src/types.rs#L12);
- [Owner's methods](https://github.com/near/core-contracts/blob/master/lockup/src/owner.rs);
- [Foundation methods](https://github.com/near/core-contracts/blob/master/lockup/src/foundation.rs);
- [View methods](https://github.com/near/core-contracts/blob/master/lockup/src/getters.rs).

## API examples

### Initialization

Initialize contract, assuming it's called from `near` account.
The lockup contract account ID is `lockup1`.
The owner account ID is `owner1`.
Lockup Duration is 365 days, starting from `2018-09-01` (`lockup_timestamp` and `release_duration` args).
Release duration is 4 years (or 1461 days including leap year).
Transfers are enabled `2020-10-13`.
Vesting is 4 years starting from `2018-09-01` to `2022-09-01` Pacific time.
Staking pool whitelist contract is at `staking-pool-whitelist`.
The foundation account ID that can terminate vesting is `near`.

Arguments in JSON format

```json
{
    "owner_account_id": "owner1",
    "lockup_duration": "0",
    "lockup_timestamp": "1535760000000000000",
    "release_duration": "126230400000000000",
    "transfers_information": {
        "TransfersEnabled": {
            "transfers_timestamp": "1602614338293769340"
        }
    },
    "vesting_schedule": {
        "VestingSchedule": {
            "start_timestamp": "1535760000000000000",
            "cliff_timestamp": "1567296000000000000",
            "end_timestamp": "1661990400000000000"
        }
    },
    "staking_pool_whitelist_account_id": "staking-pool-whitelist",
    "foundation_account_id": "near"
}
```

Command

```bash
near call lockup1 new '{"owner_account_id": "owner1", "lockup_duration": "0", "lockup_timestamp": "1535760000000000000", "release_duration": "126230400000000000", "transfers_information": {"TransfersEnabled": {"transfers_timestamp": "1602614338293769340"}}, "vesting_schedule": {"VestingSchedule": {"start_timestamp": "1535760000000000000", "cliff_timestamp": "1567296000000000000", "end_timestamp": "1661990400000000000"}}, "staking_pool_whitelist_account_id": "staking-pool-whitelist", "foundation_account_id": "near"}' --accountId=near --gas=25000000000000
```

If you need to use only lockup logic, change `vesting_schedule` parameter:
```
"vesting_schedule": None
```

If you need to use only vesting logic, change these parameters as follows:
```
"lockup_duration": 0,
"lockup_timestamp": None,
"release_duration": None
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

When the owner has accumulated some rewards on the staking pool, the contract doesn't let the owner withdraw them yet.
It's because the contract doesn't know about the accumulated rewards.
To get the new total balance for the contract, the owner has to call `refresh_staking_pool_balance`.

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

Let's say the owner checked staked balance by calling the view method on the staking pool directly and decided to unstake everything.

```bash
near call lockup1 unstake_all '{}' --accountId=owner1 --gas=125000000000000
```

#### Withdraw from the staking pool

Wait for 4 epochs (about 48 hours) and withdraw all NEAR tokens from the staking pool.

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

Once everything is unlocked and vested, the owner can add a full access key to the lockup account.
This allows the withdrawal of remaining tokens locked due to contract storage.
The owner first should generate a new key-pair (private and public keys).
Then the owner should pass the public key from this key-pair.

```bash
near call lockup1 add_full_access_key '{"new_public_key": "CE3QAXyVLeScmY9YeEyR3Tw9yXfjBPzFLzroTranYtVb"}' --accountId=owner1 --gas=50000000000000
```

Now the owner can delete this account and claim all tokens in a single operation.
WARNING: This should only be done if there are no tokens delegated to a staking pool.
Otherwise, those tokens will be lost.

This command will delete `lockup1` and transfer all remaining tokens from the lockup account to `owner1`.

```bash
near delete lockup1 owner1
```

### Vesting termination by Foundation

#### Initiate termination

To initiate termination the Foundation has to issue the following command:

```bash
near call lockup1 terminate_vesting '' --accountId=near --gas=25000000000000
```

This will block the account until the termination process is completed.

If the vesting schedule was private, the Foundation has to pass the vesting schedule, and the salt, to reveal it:

```bash
near call lockup1 terminate_vesting '"vesting_schedule_with_salt": {"vesting_schedule": {"start_timestamp": "1535760000000000000", "cliff_timestamp": "1567296000000000000", "end_timestamp": "1661990400000000000"}, salt: "cmVhbGx5X2xvbmdfYW5kX3Zlcnlfc2VjcmV0X2hhc2g="}' --accountId=near --gas=25000000000000
```

#### Monitoring status

To check the current status of the termination process, the Foundation and the owner can call:

```bash
near view lockup1 get_termination_status '{}'
```

#### Withdrawing deficit from the staking pool

If the owner staked with some staking pool and the unvested amount is larger than the current liquid balance, then it creates the deficit (otherwise the Foundation can proceed with withdrawal).

The current termination status should be `VestingTerminatedWithDeficit`.

The Foundation needs to first unstake tokens in the staking pool.
Then, once tokens become liquid, the Foundation withdraws them from the staking pool to the contract.
This is done by calling `termination_prepare_to_withdraw`.

```bash
near call lockup1 termination_prepare_to_withdraw '{}' --accountId=near --gas=175000000000000
```

The first invocation will unstake everything from the staking pool.
This should advance the termination status to `EverythingUnstaked`.
In 4 epochs, or about 48 hours, the Foundation can call the same command again:

```bash
near call lockup1 termination_prepare_to_withdraw '{}' --accountId=near --gas=175000000000000
```

If everything went okay, the status should be advanced to `ReadyToWithdraw`.

### Withdrawing from the account

Once the termination status is `ReadyToWithdraw`, the Foundation can proceed with withdrawing the unvested balance.

```bash
near call lockup1 termination_withdraw '{"receiver_id": "near"}' --accountId=near --gas=75000000000000
```

In case of successful withdrawal, the unvested balance will become `0` and the owner can use this contract again.

## Change Log

### `3.1.0`

- Reduced minimum required balance for the lockups from 35 NEAR to 3.5 NEAR;
- Improved the documentation.

### `3.0.0`

- Release duration now starts from the moment the tokens are unlocked.
  The tokens are unlocked at the following timestamp `max(transfers_enabled_timestamp + lockup_duration, lockup_timestamp)`.
  NOTE: If the `lockup_timestamp` is not specified, the tokens are unlocked at `transfers_enabled_timestamp + lockup_duration`.

### `2.0.0`

- Changed `vesting_schedule` initialization argument to allow it to hide the vesting schedule behind a hash to keep it private.
- Added view method `get_vesting_information` to view internal vesting information.

### `1.0.0`

- Make `release_duration` independent from the `vesting_schedule`. They are not allowed to be used simultaneously.
- Internal. Remove some JSON serialization on inner structures.
- Fix a bug with the prepaid gas exceeded during the foundation callback by increasing base gas.
- Include the minimum amount of gas needed for every call.
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
  This allows being more flexible with the account access including multi-sig implementation.
- The lockup contract account should not have any access keys until the account is fully vested and unlocked.
  Only then the owner can add the full access key.
- Removed methods for adding and removing staking/main access keys.
- Added a view method to get the account ID of the owner.
