# Exchange deposit receiver

## Background

Exchanges usually create a new account for each a user to deposit their tokens. On NEAR, the account cannot be created
without a deposit to cover the occupied storage. Thus it costs the exchange tokens to create a new account before the user
can deposit tokens. So if the user did not deposit tokens into the account, the exchange spent tokens on the creation of
the account.

To avoid creating the account before any tokens from the user are received, the exchanges can also receive tokens
directly into their hot wallet, but every deposit has to be marked to be able to properly attribute it to the owner.
This is usually done with an additional `memo` field in the transaction. Although NEAR doesn't support comments on the transfers, it does have function calls which can have tokens attached.

## Overview

The goal of this contract is to provide a contract endpoint for depositing tokens with a memo.

### The process is the following:
- An exchange deploys this contract on their hot wallet account. The exchange also maintains the full access to this account by
having a full access key on this account. It allows exchange to withdraw tokens from this account.
- When a user wants to deposit tokens to their account on the exchange, the exchange generates a unique `<MEMO>` message for
a user.
- The user sends a specific transaction to the exchange's hot wallet account with a single action.
This action should be supported by the wallets and the lockup contract.
```rust
FunctionCall {
    method_name: "exchange_deposit",
    args: b"<MEMO>",
    gas: ...,
    deposit: <DEPOSIT_AMOUNT>,
}
```
- The transaction gets executed and the exchange's hot wallet account receives the `<DEPOSIT_AMOUNT>`.
- Exchange monitors incoming transactions and sees that this deposit has been associated with the `<MEMO>`.
- The exchange can now attribute `<DEPOSIT_AMOUNT>` for the user's account by mapping it with the received `<MEMO>`.

## Interface

```rust
pub fn exchange_deposit();
```

## API examples

First create an account. Here we use `coin_exchange` as an example

```bash
near create_account coin_exchange --masterAccount=<account>
```

Now deploy the contract

```bash
near deploy --wasmFile=<contract_binary> --accountId=coin_exchange
```

Send `10` NEAR tokens to the exchange account `coin_exchange` with memo `USER_123` from user `token_owner`.

Command:

```bash
near call coin_exchange exchange_deposit 'USER_123' --accountId=token_owner --amount=10
```

NOTE: This command temporarily doesn't work due to near-cli assumption about JSON input format. See https://github.com/near/near-cli/issues/503
As a temporary workaround, the above command can be changed to
```bash
near call coin_exchange exchange_deposit '{"": "USER_123"}' --accountId=token_owner --amount=10
```
