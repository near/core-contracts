# Staking Pool Factory Contract

This contract deploys and automatically whitelists new staking pool contracts.
It allows any user to create a new whitelisted staking pool.

The staking pool factory contract packages the binary of the staking pool contract within its own binary.
To create a new staking pool a user should issue a function call transaction and attach the required minimum deposit.
The entire deposit will be transferred to the newly created staking pool contract in order to cover the required storage.

When a user issues a function call towards the factory to create a new staking pool the factory internally checks that
the staking pool account ID does not exists, validates arguments for the staking pool initialization and then issues a
receipt that creates the staking pool. Once the receipt executes, the factory checks the status of the execution in the
callback. If the staking pool was created successfully, the factory then whitelists the newly created staking pool.
Otherwise, the factory returns the attached deposit back the users and returns `false`.

## Changelog

### `0.1.1`

- Rebuild with the staking pool contract version of `0.2.1`.

## API

```rust
/// Initializes the staking pool factory with the given account ID of the staking pool whitelist
/// contract.
#[init]
pub fn new(staking_pool_whitelist_account_id: AccountId) -> Self;

/// Returns the minimum amount of tokens required to attach to the function call to
/// create a new staking pool.
pub fn get_min_attached_balance(&self) -> U128;

/// Returns the total number of the staking pools created from this factory.
pub fn get_number_of_staking_pools_created(&self) -> u64;

/// Creates a new staking pool.
/// - `staking_pool_id` - the prefix of the account ID that will be used to create a new staking
///    pool account. It'll be prepended to the staking pool factory account ID separated by dot.
/// - `owner_id` - the account ID of the staking pool owner. This account will be able to
///    control the staking pool, set reward fee, update staking key and vote on behalf of the
///     pool.
/// - `stake_public_key` - the initial staking key for the staking pool.
/// - `reward_fee_fraction` - the initial reward fee fraction for the staking pool.
#[payable]
pub fn create_staking_pool(
    &mut self,
    staking_pool_id: String,
    owner_id: AccountId,
    stake_public_key: Base58PublicKey,
    reward_fee_fraction: RewardFeeFraction,
) -> Promise;

/// Callback after a staking pool was created.
/// Returns the promise to whitelist the staking pool contract if the pool creation succeeded.
/// Otherwise refunds the attached deposit and returns `false`.
pub fn on_staking_pool_create(
    &mut self,
    staking_pool_account_id: AccountId,
    attached_deposit: U128,
    predecessor_account_id: AccountId,
) -> PromiseOrValue<bool>;
```
