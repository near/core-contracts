# wNear NEP21 Token contract

NEP21 is based on:
 
    https://github.com/near/near-sdk-rs/blob/ab5c01ca4c61a6414484b69302b84e5ce3113f2f/examples/fungible-token/src/lib.rs

The aim of the contract is to enable the wrapping of the native Ⓝ token into a NEP21 compatible token.
It's the equivalent of wrapping ETH into wETH via the WETH. This contract is based on the functionality 
of the WETH9 and WETH10 Solidity smart contracts.

## Minting wNear

The following methods are available for minting:
* `deposit(amount)`
* `deposit_to(receiver_id, amount)`

`deposit(amount)` just proxies through to `deposit_to(receiver_id, amount)` where receiver_id will be set to `env::predecessor_account_id()`.

When using `deposit_to`, the following requirements apply:
* receiver_id cannot be the wNear contract - to stop people accidentally losing money
* receiver_id needs to be a valid account Id
* Amount must be a positive integer

Both deposit methods will require an attached deposit that covers the storage requirements and the amount of `wNear` tokens being minted.

## Withdrawing Near

The following methods are available for unwrapping wNear:
* `withdraw(amount)`
* `withdraw_to(receiver_id, amount)`
* `withdraw_from(owner_id, receiver_id, amount)`

Like `deposit()`, `withdraw()` is the simplest and can be called by the owner of `wNear` tokens to claim the underlying Near Ⓝ asset. Further, `withdraw()` just proxies through to `withdraw_to()` where receiver_id will be set to `env::predecessor_account_id()`.

When using `withdraw_to`, the following requirements apply:
* receiver_id cannot be the wNear contract - to stop people accidentally losing money
* receiver_id needs to be a valid account Id
* Amount must be a positive integer

Caller must have a balance that is greater than or equal to `amount`.

`withdraw_from` (like `withdraw_to`) comes from WETH10 and allows an approved account to unwrap `wNear` from an `owner` who has a `wNear` balance. The approved account can then send the underlying Ⓝ to a receiver account of their choosing.

## Taking wNear for a testdrive

`wNear` is deployed at `wnear.testnet`.

Here is an example near cli command that would mint 50 `wNear`:
```
near call wnear.testnet deposit '{"amount": "50000000000000000000000000"}' --accountId <your_testnet_account> --amount 50
```

## Running unit tests

Unit tests can be run with the following command:

`cargo test -p w_near --lib`

You should see this output:
```
TODO
```
