# Exchange deposit receiver

## Background

Exchanges usually creates an new account for a user to deposit their tokens. On NEAR, the account can't be created
without a deposit to cover the occupied storage. It cost tokens on exchange side to create such account before the user
can deposit tokens. So if the user doesn't deposit tokens to the account, the exchange spent tokens on the creation of
the account.

To avoid creating the account before any tokens from the user are received, the exchanges can also receive tokens
directly into their hot wallet, but every deposit has to be marked to be able to properly attribute it to the owner.
It's usually done with an additional `memo` field in the transaction. NEAR doesn't support comments on the transfers.
But NEAR does have function calls and the ability to attach tokens with the function call.

## Overview

The goal of this contract is to provide a contract endpoint to be able to deposit tokens with a memo.

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

Send `10` NEAR tokens to the exchange account `coin_exchange` with memo `USER_123` from user `token_owner`.

Command:

```bash
near call coin_exchange exchange_deposit 'USER_123' --accountId=token_owner --amount=10
```

NOTE: This command temporarily doesn't work due to near-cli assumption about JSON input format. See https://github.com/near/near-cli/issues/503
