# Staking Pool Factory Contract

This contract allows to deploy new staking pool contract and automatically whitelist them.
It allows any user to create an new whitelisted staking pool.

The staking pool factory contract keeps the hardcoded binary of the staking pool contract within itself.
To create a new staking pool a user should issue a function call transaction and attach the required minimum deposit.
The entire deposit will be transferred to the newly created staking pool contract in order to cover the required storage.

When a user issues a function call towards the factory to create a new staking pool the factory internally checks that
the staking pool account ID doesn't exists, validates arguments for the staking pool initialization and then issues a
receipt that creates the staking pool. Once the receipt executes, the factory checks the status of the execution in the
callback. If the staking pool was created successfully, the factory then whitelists the newly created staking pool.
Otherwise, the factory returns the attached deposit back the users and returns `false`.

## API

```rust
/// Initializes the staking pool factory with the given account ID of the staking pool whitelist
// contract.
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
) -> Promise {
    assert!(
        env::attached_deposit() >= MIN_ATTACHED_BALANCE,
        "Not enough attached deposit to complete staking pool creation"
    );

    assert!(
        staking_pool_id.find('.').is_none(),
        "The staking pool ID can't contain `.`"
    );

    let staking_pool_account_id = format!("{}.{}", staking_pool_id, env::current_account_id());
    assert!(
        env::is_valid_account_id(staking_pool_account_id.as_bytes()),
        "The staking pool account ID is invalid"
    );

    assert!(
        env::is_valid_account_id(owner_id.as_bytes()),
        "The owner account ID is invalid"
    );
    reward_fee_fraction.assert_valid();

    assert!(
        self.staking_pool_account_ids
            .insert(&staking_pool_account_id),
        "The staking pool account ID already exists"
    );

    Promise::new(staking_pool_account_id.clone())
        .create_account()
        .transfer(env::attached_deposit())
        .deploy_contract(include_bytes!("../../staking-pool/res/staking_pool.wasm").to_vec())
        .function_call(
            b"new".to_vec(),
            serde_json::to_vec(&StakingPoolArgs {
                owner_id,
                stake_public_key,
                reward_fee_fraction,
            })
            .unwrap(),
            NO_DEPOSIT,
            gas::STAKING_POOL_NEW,
        )
        .then(ext_self::on_staking_pool_create(
            staking_pool_account_id,
            env::attached_deposit().into(),
            env::predecessor_account_id(),
            &env::current_account_id(),
            NO_DEPOSIT,
            gas::CALLBACK,
        ))
}

/// Callback after a staking pool was created.
/// Returns the promise to whitelist the staking pool contract if the pool creation succeeded.
/// Otherwise refunds the attached deposit and returns `false`.
pub fn on_staking_pool_create(
    &mut self,
    staking_pool_account_id: AccountId,
    attached_deposit: U128,
    predecessor_account_id: AccountId,
) -> PromiseOrValue<bool> {
    assert_self();

    let staking_pool_created = is_promise_success();

    if staking_pool_created {
        env::log(
            format!(
                "The staking pool @{} was successfully created. Whitelisting...",
                staking_pool_account_id
            )
            .as_bytes(),
        );
        ext_whitelist::add_staking_pool(
            staking_pool_account_id,
            &self.staking_pool_whitelist_account_id,
            NO_DEPOSIT,
            gas::WHITELIST_STAKING_POOL,
        )
        .into()
    } else {
        self.staking_pool_account_ids
            .remove(&staking_pool_account_id);
        env::log(
            format!(
                "The staking pool @{} creation has failed. Returning attached deposit of {} to @{}",
                staking_pool_account_id,
                attached_deposit.0,
                predecessor_account_id
            ).as_bytes()
        );
        Promise::new(predecessor_account_id).transfer(attached_deposit.0);
        PromiseOrValue::Value(false)
    }
}
```
