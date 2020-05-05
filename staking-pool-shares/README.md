# Example Staking / Delegation contract

*This is an experimental contract. Please use only on TestNet.*

This contract provides a way for other users to delegate funds to a single staker.

Implements the https://github.com/nearprotocol/NEPs/pull/27 standard.

## Pre-requisites

To develop Rust contracts you would need to:
* Install [Rustup](https://rustup.rs/):
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
* Add wasm target to your toolchain:
```bash
rustup target add wasm32-unknown-unknown
```

## Building the contract

```bash
./build.sh
```

## Usage

Commands to deploy and initialize a staking contract:

```bash
near create_account my_validator --masterAccount=owner
near deploy --accountId=my_validator --wasmFile=res/staking_pool_with_shares.wasm
# Initialize staking pool at account `my_validator` for the owner account ID `owner`, given staking pool and 10% reward fee.
near call my_validator new '{"owner_id": "owner", "stake_public_key": "CE3QAXyVLeScmY9YeEyR3Tw9yXfjBPzFLzroTranYtVb", "reward_fee_fraction": {"numerator": 10, "denominator": 100}}' --account_id owner
# TODO: Delete all access keys from the `my_validator` account
```

As a user, to delegate money:

```bash
near call my_validator deposit '{}' --accountId user1 --amount 100
near call my_validator stake '{"amount": "100000000000000000000000000"}' --accountId user1
```

To update current rewards:

```bash
near call my_validator ping '{}' --accountId user1
```

View methods:

```bash
# User1 total balance
near view my_validator get_account_total_balance '{"account_id": "user1"}'

# User1 staked balance
near view my_validator get_account_staked_balance '{"account_id": "user1"}'

# User1 unstaked balance
near view my_validator get_account_unstaked_balance '{"account_id": "user1"}'

# Whether user1 can withdraw now
near view my_validator is_account_unstaked_balance_available '{"account_id": "user1"}'

# Total staked balance of the entire pool
near view my_validator get_total_staked_balance '{}'

# Owner of the staking pool
near view my_validator get_owner_id '{}'

# Current reward fee
near view my_validator get_reward_fee_fraction '{}'

# Owners balance
near view my_validator get_account_total_balance '{"account_id": "owner"}'
```

To un-delegate, first run `unstake`:

```bash
near call my_validator unstake '{"amount": "100000000000000000000000000"}' --accountId user1
```

And after 3 epochs, run `withdraw`:

```bash
near call my_validator withdraw '{"amount": "100000000000000000000000000"}' --accountId user1
```

## Interface

```rust
pub struct RewardFeeFraction {
    pub numerator: u32,
    pub denominator: u32,
}

/// Initializes the contract with the given owner_id, initial staking public key (with ED25519
/// curve) and initial reward fee fraction that owner charges for the validation work.
#[init]
pub fn new(
    owner_id: AccountId,
    stake_public_key: Base58PublicKey,
    reward_fee_fraction: RewardFeeFraction,
);

/// Call to distribute rewards after the new epoch. It's automatically called before every
/// action.
pub fn ping(&mut self);

/// Deposits the attached amount into the inner account of the predecessor.
#[payable]
pub fn deposit(&mut self);

/// Withdraws the non staked balance for given account.
/// It's only allowed if the `unstake` action was not performed in the recent 3 epochs.
pub fn withdraw(&mut self, amount: U128);

/// Stakes the given amount from the inner account of the predecessor.
/// The inner account should have enough unstaked balance.
pub fn stake(&mut self, amount: U128);

/// Unstakes the given amount from the inner account of the predecessor.
/// The inner account should have enough staked balance.
/// The new total unstaked balance will be available for withdrawal in 3 epochs.
pub fn unstake(&mut self, amount: U128);

/****************/
/* View methods */
/****************/

/// Returns the unstaked balance of the given account.
pub fn get_account_unstaked_balance(&self, account_id: AccountId) -> U128;

/// Returns the staked balance of the given account.
/// NOTE: This is computed from the amount of "stake" shares the given account has and the
/// current amount of total staked balance and total stake shares on the account.
pub fn get_account_staked_balance(&self, account_id: AccountId) -> U128;

/// Returns the total balance of the given account (including staked and unstaked balances).
pub fn get_account_total_balance(&self, account_id: AccountId) -> U128;

/// Returns `true` if the given account can withdraw tokens in the current epoch.
pub fn is_account_unstaked_balance_available(&self, account_id: AccountId) -> bool;

/// Returns the total staking balance.
pub fn get_total_staked_balance(&self) -> U128;

/// Returns account ID of the staking pool owner.
pub fn get_owner_id(&self) -> AccountId;

/// Returns the current reward fee as a fraction.
pub fn get_reward_fee_fraction(&self) -> RewardFeeFraction;

/*******************/
/* Owner's methods */
/*******************/

/// Owner's method.
/// Updates current public key to the new given public key.
pub fn update_staking_key(&mut self, stake_public_key: Base58PublicKey);

/// Owner's method.
/// Updates current reward fee fraction to the new given fraction.
pub fn update_reward_fee_fraction(&mut self, reward_fee_fraction: RewardFeeFraction);
```
