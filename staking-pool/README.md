# Staking / Delegation contract

This contract provides a way for other users to delegate funds to a single validation node.

Implements the https://github.com/nearprotocol/NEPs/pull/27 standard.

There are three different roles:
- The staking pool contract account `my_validator`. A key-less account with the contract that pools funds.
- The owner of the staking contract `owner`. Owner runs the validator node on behalf of the staking pool account.
- Delegator accounts `user1`, `user2`, etc. Accounts that want to stake their funds with the pool.

The owner can setup such contract and validate on behalf of this contract in their node.
Any other user can send their tokens to the contract, which will be pooled together and increase the total stake.
These users accrue rewards (subtracted fees set by the owner).
Then they can unstake and withdraw their balance after some unlocking period.

## Staking pool implementation details

For secure operation of the staking pool, the contract should not have any access keys.
Otherwise the contract account may issue a transaction that can violate the contract guarantees.

After users deposit tokens to the contract, they can stake some or all of them to receive "stake" shares.
The price of a "stake" share can be defined as the total amount of staked tokens divided by the the total amount of "stake" shares.
The number of "stake" shares is always less than the number of the staked tokens, so the price of single "stake" share is not less than `1`.

### Initialization

A contract has to be initialized with the following parameters:
- `owner_id` - `string` the account ID of the contract owner. This account will be able to call owner-only methods. E.g. `owner`
- `stake_public_key` - `string` the initial public key that will be used for staking on behalf of the contract's account in base58 ED25519 curve. E.g. `KuTCtARNzxZQ3YvXDeLjx83FDqxv2SdQTSbiq876zR7`
- `reward_fee_fraction` - `json serialized object` the initial value of the fraction of the reward that the owner charges delegators for running the node.
The fraction is defined by the numerator and denumerator with `u32` types. E.g. `{numerator: 10, denominator: 100}` defines `10%` reward fee.
The fraction can be at most `1`. The denumerator can't be `0`.

During the initialization the contract checks validity of the input and initializes the contract.
The contract shouldn't have locked balance during the initialization.

At the initialization the contract allocates one trillion yocto NEAR tokens towards "stake" share price guarantees.
This fund is later used to adjust the the amount of staked and unstaked tokens due to rounding error.
For each stake and unstake action, the contract may spend at most 1 yocto NEAR from this fund (implicitly).

The current total balance (except for the "stake" share price guarantee amount) is converted to shares and will be staked (after the next action).
This balance can never be unstaked or withdrawn from the contract.
It's used to maintain the minimum number of shares, as well as help pay for the potentially growing contract storage.

### Delegator accounts

The contract maintains account information per delegator associated with the hash of the delegator's account ID.

The information contains:
- Unstaked balance of the account.
- Number of "stake" shares.
- The minimum epoch height when the unstaked balance can be withdrawn. Initially zero.

A delegator can do the following actions:

#### Deposit

When a delegator account first deposits funds to the contract, the internal account is created and credited with the
attached amount of unstaked tokens.

#### Stake

When an account wants to stake a given amount, the contract calculates the number of "stake" shares (`num_shares`) and the actual rounded stake amount (`amount`).
The unstaked balance of the account is decreased by `amount`, the number of "stake" shares of the account is increased by `num_shares`.
The contract increases the total number of staked tokens and the total number of "stake" shares. Then the contract restakes.

#### Unstake

When an account wants to unstake a given amount, the contract calculates the number of "stake" shares needed (`num_shares`) and
the actual required rounded unstake amount (`amount`). It's calculated based on the current total price of "stake" shares.
The unstaked balance of the account is increased by `amount`, the number of "stake" shares of the account is decreased by `num_shares`.
The minimum epoch height when the account can withdraw is set to the current epoch height increased by `4`.
The contract decreases the total number of staked tokens and the total number of "stake" shares. Then the contract restakes.

#### Withdraw

When an account wants to withdraw, the contract checks the minimum epoch height of this account and checks the amount.
Then sends the transfer and decreases the unstaked balance of the account.

#### Ping

Calls the internal function to distribute rewards if the blockchain epoch switched. The contract will restake in this case.

### Reward distribution

Before every action the contract calls method `internal_ping`.
This method distributes rewards towards active delegators when the blockchain epoch switches.
The rewards might be given due to staking and also because the contract earns gas fee rebates for every function call.
Note, the if someone accidentally (or intentionally) transfers tokens to the contract (without function call), then
tokens from the transfer will be distributed to the active stake participants of the contract in the next epoch.
Note, in a rare scenario, where the owner withdraws tokens and while the call is being processed deletes their account, the
withdraw transfer will fail and the tokens will be returned to the staking pool. These tokens will also be distributed as
a reward in the next epoch.

The method first checks that the current epoch is different from the last epoch, and if it's not changed exits the method.

The reward are computed the following way. The contract keeps track of the last known total account balance.
This balance consist of the initial contract balance, and all delegator account balances (including the owner) and all accumulated rewards.
(Validation rewards are added automatically at the beginning of the epoch, while contract execution gas rebates are added after each transaction)

When the method is called the contract uses the current total account balance (without attached deposit) and the subtracts the last total account balance.
The difference is the total reward that has to be distributed.

The fraction of the reward is awarded to the contract owner. The fraction is configurable by the owner, but can't exceed 100%.
Note, that it might be unfair for the participants of the pool if the owner changes reward fee. But this owner will lose trust of the
participants and it will lose future revenue in the long term. This should be enough to prevent owner from abusing reward fee.
It could also be the case that they could change the reward fee to make their pool more attractive.

The remaining part of the reward is added to the total staked balance. This action increases the price of each "stake" share without
changing the amount of "stake" shares owned by different accounts. Which is effectively distributing the reward based on the number of shares.

The owner's reward is converted into "stake" shares at the new price and added to the owner's account.
It's done similarly to `stake` method but without debiting the unstaked balance of owner's account.

Once the rewards are distributed the contract remembers the new total balance.

## Owner-only methods

Contract owner can do the following:
- Change public staking key. This action restakes with the new key.
- Change reward fee fraction.
- Vote on behalf of the pool. This is needed for the NEAR chain governance, and can be discussed in the following NEP: https://github.com/nearprotocol/NEPs/pull/62
- Pause and resume staking. When paused, the pool account unstakes everything (stakes 0) and doesn't restake.
It doesn't affect the staking shares or reward distribution. Pausing is useful for node maintenance. Note, the contract is not paused by default.

## Staking pool contract guarantees and invariants

This staking pool implementation guarantees the required properties of the staking pool standard:

- The contract can't lose or lock tokens of users.
- If a user deposited X, the user should be able to withdraw at least X.
- If a user successfully staked X, the user can unstake at least X.
- The contract should not lock unstaked funds for longer than 4 epochs after unstake action.

It also has inner invariants:

- The staking pool contract is secure if it doesn't have any access keys.
- The price of a "stake" is always at least `1`.
- The price of a "stake" share never decreases.
- The reward fee is a fraction be from `0` to `1` inclusive.
- The owner can't withdraw funds from other delegators.
- The owner can't delete the staking pool account.

NOTE: Guarantees are based on the no-slashing condition. Once slashing is introduced, the contract will no longer
provide some guarantees. Read more about slashing in [Nightshade paper](https://near.ai/nightshade).

## Changelog

### `0.4.0`

- Internal refactoring. Moving internal methods to `internal.rs`
- Added 4 new delegator methods:
    - `deposit_and_stake` - to deposit and stake attached balance in one call.
    - `stake_all` - to stake all unstaked balance.
    - `unstake_all` - to unstake all staked balance.
    - `withdraw_all` - to withdraw all unstaked balance.


### `0.3.0`

- Inner implementation has changed from using the hash of the account ID to use unmodified account ID as a key.
- Added 3 new view methods:
    - `get_account` - Returns human readable representation of the account for the given account ID
    - `get_number_of_accounts` - returns the total number of accounts that have positive balance in this staking pool.
    - `get_accounts` - Returns up to the limit of accounts starting from the given offset


### `0.2.1`

- Update `vote` interface to match the voting contract interface.


### `0.2.0`

- Added new owners methods: `pause_staking` and `resume_staking`. Allows pool owner to unstake everything from the pool for node maintenance.
- Added a new view method `is_staking_paused` to check whether the pool has paused staking.


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
near deploy --accountId=my_validator --wasmFile=res/staking_pool.wasm
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

# Staking key
near view my_validator get_staking_key '{}'
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

/// Distributes rewards and restakes if needed.
pub fn ping(&mut self);

/// Deposits the attached amount into the inner account of the predecessor.
#[payable]
pub fn deposit(&mut self);

 /// Deposits the attached amount into the inner account of the predecessor and stakes it.
#[payable]
pub fn deposit_and_stake(&mut self);

/// Withdraws the non staked balance for given account.
/// It's only allowed if the `unstake` action was not performed in the four most recent epochs.
pub fn withdraw(&mut self, amount: U128);

/// Withdraws the entire unstaked balance from the predecessor account.
/// It's only allowed if the `unstake` action was not performed in the four most recent epochs.
pub fn withdraw_all(&mut self);

/// Stakes the given amount from the inner account of the predecessor.
/// The inner account should have enough unstaked balance.
pub fn stake(&mut self, amount: U128);

/// Stakes all available unstaked balance from the inner account of the predecessor.
pub fn stake_all(&mut self);

/// Unstakes the given amount from the inner account of the predecessor.
/// The inner account should have enough staked balance.
/// The new total unstaked balance will be available for withdrawal in four epochs.
pub fn unstake(&mut self, amount: U128);

/// Unstakes all staked balance from the inner account of the predecessor.
/// The new total unstaked balance will be available for withdrawal in four epochs.
pub fn unstake_all(&mut self);

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

/// Returns the staking public key
pub fn get_staking_key(&self) -> Base58PublicKey;

/// Returns true if the staking is paused
pub fn is_staking_paused(&self) -> bool;

/// Returns human readable representation of the account for the given account ID.
pub fn get_account(&self, account_id: AccountId) -> HumanReadableAccount;

/// Returns the number of accounts that have positive balance on this staking pool.
pub fn get_number_of_accounts(&self) -> u64;

/// Returns the list of accounts
pub fn get_accounts(&self, from_index: u64, limit: u64) -> Vec<HumanReadableAccount>;

/*******************/
/* Owner's methods */
/*******************/

/// Owner's method.
/// Updates current public key to the new given public key.
pub fn update_staking_key(&mut self, stake_public_key: Base58PublicKey);

/// Owner's method.
/// Updates current reward fee fraction to the new given fraction.
pub fn update_reward_fee_fraction(&mut self, reward_fee_fraction: RewardFeeFraction);

/// Owner's method.
/// Calls `vote(is_vote)` on the given voting contract account ID on behalf of the pool.
pub fn vote(&mut self, voting_account_id: AccountId, is_vote: bool) -> Promise;

/// Owner's method.
/// Pauses pool staking.
pub fn pause_staking(&mut self);

/// Owner's method.
/// Resumes pool staking.
pub fn resume_staking(&mut self);
```

## Migrating from an existing validator or contract
This provides instructions to migrate your staked validator or a validator contract to a new contract

#### Upgrade to the latest near-shell:
```bash
npm install -g near-shell
```
#### Set Environment and Login:

##### If not logged into the browser, recover your account with the seed phrase first
https://wallet.betanet.nearprotocol.com/create/

```bash
#Set the NEAR environment to the target network (betanet,testnet,mainnet)
export NEAR_ENV=betanet

near login
```

#### Unstake and Withdraw:
```bash
#If you staked to your validator unstake, there is no withdraw
near stake nearkat.betanet <staking public key> 0

#If you staked to a contract get the staked balance
near view my_validator get_account_staked_balance '{"account_id": "user1"}'

#Unsake by copying and pasting the staked balance
near call my_validator unstake '{"amount": "100000000000000000000000000"}' --accountId user1

#Wait 4 epochs (12 hours) to withdraw and check if balance is available to withdraw
near view my_validator is_account_unstaked_balance_available '{"account_id": "user1"}'

#If is_account_unstaked_balance_available returns "true" withdraw
near call my_validator withdraw '{"amount": "100000000000000000000000000"}' --accountId user1
```
#### Download new contract with Git:
```bash
mkdir staking-pool

cd staking-pool

git clone https://github.com/near/initial-contracts

cd initial-contracts

cd staking-pool
```
#### Build contract with Rust (This step is optional since the contract is compiled):
##### Install Rust:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

#Add rust to current shell path
source $HOME/.cargo/env
```
##### Add wasm target to your toolchain:
```bash
rustup target add wasm32-unknown-unknown
```
##### Build:
```bash
./build.sh
```
#### Create a new account to deploy contract to
- Set my_validator to the name you want publicly displayed
- --masterAccount is your account you signed up to StakeWars2 with
```bash
near create_account my_validator --masterAccount=owner
```
#### Deploy the contract to the new account
```bash
near deploy --accountId=my_validator --wasmFile=res/staking_pool.wasm
```
#### Create a new node:

**Note** after you NEAR is unstaked stop your node and create a new one to run as the contract account

##### Stop your node
```bash
nearup stop
```
##### Move your ~/.near/betanet folder, to remove references to any previous validator node
```bash
mv ~/.near/betanet ~/.near/betanet_old
```
##### Launch your new node
With the command nearup betanet. Modify the launch command according to your actual validator configuration (e.g. using --nodocker and --binary-path)

##### Set your validator ID.
Put your staking pool account (the one we called my_validator in the steps above)

##### Copy your validator public key, or issue the command (before the next step)
```bash
cat ~/.near/betanet/validator_key.json |grep "public_key"
```
#### Initialize staking pool at account `my_validator` for the owner account ID `owner`, given staking pool and 10% reward fee
```bash
near call my_validator new '{"owner_id": "owner", "stake_public_key": "CE3QAXyVLeScmY9YeEyR3Tw9yXfjBPzFLzroTranYtVb", "reward_fee_fraction": {"numerator": 10, "denominator": 100}}' --account_id owner
```
#### Check the current `seat price` to transfer the correct amount to your delegator(s)
```bash
near validators next| grep "seat price"
```
#### Register a delegator account (repeat these steps for additional delegators)
-- https://wallet.betanet.near.org
-- backup your seed phrase
-- transfer NEAR from your MasterAccount to the delegator account

#### Login and authorize the delegator
```bash
near login
```
#### Deposit NEAR from the delegator account to the valdiator contract
```bash
near call my_validator deposit '{}' --accountId user1 --amount 100
```
#### Stake the deposited amount to the validator contract
```bash
near call my_validator stake '{"amount": "100000000000000000000000000"}' --accountId user1
```
#### Check that your validator proposal was (Accepted) or deposit and stake more NEAR
```bash
near proposals | grep my_validator
#After some time check to make sure you're listed
near validators next | grep my_validator
```
## Common errors and resolutions

#### ERROR while adding wasm32 to toolchain: error[E0463]: can't find crate for `core`
You might have a nightly version of cargo, rustc, rustup, update to stable
```bash
rustup update stable

#Install target with stable version of Rustup
rustup +stable target add wasm32-unknown-unknown
```

#### Error:  TypedError: [-32000] Server error: account <accountId> does not exist while viewing
You are not logged in
```bash
near login
```

#### Error:  GasExceeded [Error]: Exceeded the prepaid gas
Add additional gas by adding the parameter: --gas 10000000000000000

#### Error: "wasm execution failed with error: FunctionCallError(MethodResolveError(MethodNotFound))"
Your function call is incorrect or your contract is not updated


