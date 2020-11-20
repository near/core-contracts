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
running 35 tests
test w_near_tests::default_fails ... ok
test w_near_tests::contract_creation_with_new ... ok
test w_near_tests::deposit_fails_when_amount_is_zero ... ok
test w_near_tests::deposit ... ok
test w_near_tests::deposit_to_fails_when_recipient_is_invalid ... ok
test w_near_tests::deposit_to_bob_from_carol ... ok
test w_near_tests::deposit_to_fails_when_recipient_is_w_near_contract ... ok
test w_near_tests::deposit_to_fails_when_the_required_deposit_is_not_attached ... ok
test w_near_tests::saturating_dec_allowance ... ok
test w_near_tests::carol_escrows_to_bob_transfers_to_alice ... ok
test w_near_tests::carol_escrows_to_bob_locks_and_transfers_to_alice ... ok
test w_near_tests::self_allowance_fail_no_deposit ... ok
test w_near_tests::self_dec_allowance_fail ... ok
test w_near_tests::self_inc_allowance_fail ... ok
test w_near_tests::saturating_inc_allowance ... ok
test w_near_tests::self_allowance_set_for_refund ... ok
test w_near_tests::simple_deposit_by_carol_and_withdrawal_to_alice ... ok
test w_near_tests::simple_deposit_and_withdrawal ... ok
test w_near_tests::transfer_fail_self ... ok
test w_near_tests::transfer_after_deposit ... ok
test w_near_tests::withdraw_fails_when_withdrawal_amount_is_zero ... ok
test w_near_tests::transfer_fail_to_contract ... ok
test w_near_tests::withdraw_from_fails_when_the_owner_and_recipient_are_the_same ... ok
test w_near_tests::withdraw_from_fails_when_the_recipient_is_invalid ... ok
test w_near_tests::withdraw_from_fails_when_the_escrow_account_does_not_have_enough_allowance ... ok
test w_near_tests::withdraw_from_fails_when_the_recipient_is_the_w_near_contract ... ok
test w_near_tests::withdraw_from_fails_when_the_withdrawal_amount_is_zero ... ok
test w_near_tests::transfer_with_infinite_allowance_should_not_reduce_allowance ... ok
test w_near_tests::withdraw_from_fails_when_trying_to_withdraw_more_than_owners_balance ... ok
test w_near_tests::withdraw_to_fails_when_carol_tries_to_withdraw_more_than_her_w_near_balance ... ok
test w_near_tests::withdraw_from_with_correct_allowance_should_be_successful ... ok
test w_near_tests::withdraw_to_fails_when_recipient_is_invalid ... ok
test w_near_tests::withdraw_from_with_infinite_allowance_should_not_reduce_allowance ... ok
test w_near_tests::withdraw_to_fails_when_recipient_is_w_near_contract ... ok
test w_near_tests::withdraw_from_with_infinite_allowance_should_not_withdraw_more_than_balance ... ok
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```
