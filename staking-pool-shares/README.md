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
near create_account my_validator
near deploy --accountId=my_validator --wasmFile=res/staking_contract.wasm
near call my_validator new '{"owner": "my_validator", "stake_public_key": "CE3QAXyVLeScmY9YeEyR3Tw9yXfjBPzFLzroTranYtVb"}' --account_id my_validator
```

As a user, to delegate money:

```bash
near call my_validator deposit '' --accountId user1 --amount 100
near call my_validator stake '{"amount": "100000000000000000000000000"}' --accountId user1
```

To un-delegate, first run `unstake`:

```bash
near call my_validator unstake '{"amount": "100000000000000000000000000"}' --accountId user1
```

And after 3 epochs, run `withdraw`:

```bash
near call my_validator withdraw '{"amount": "100000000000000000000000000"}' --accountId user1
```
