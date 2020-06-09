# Basic Mutlisig contract

*This is an experimental contract. Please use only on TestNet.*

This contract provides:
 - Set K out of N multi sig scheme
 - Request to sign transfers, function calls, adding and removing keys.
 - Any of the access keys can confirm, until the required number of confirmation achieved.

## Multisig implementation details

Mutlisig uses set of `FunctionCall` `AccessKey`s as a set of allowed N keys. 
When contract is being setup, it should be initialized with set of keys that will be initially managing this account.
All operations going forward will require `K` signatures to be performed.

### Initialization

### Request

There are number of different request types that multisig can confirm and execute:
```rust
 Transfer { receiver_id: AccountId, amount: U128 }
 AddKey { public_key: Base58PublicKey },
 DeleteKey { public_key: Base58PublicKey },
 FunctionCall {
        contract_id: AccountId,
        method_name: String,
        args: Base64VecU8,
        deposit: U128,
        gas: Gas
    },
 SetNumConfirmations { num_confirmations: u32 }
``` 

### Methods

```rust
/// Add request for multisig.
pub fn add_request(&mut self, request: MultiSigRequest) -> RequestId {

/// Remove given request and associated confirmations.
pub fn delete_request(&mut self, request_id: RequestId) {

/// Confirm given request with given signing key.
/// If with this, there has been enough confirmation, a promise with request will be scheduled.
pub fn confirm(&mut self, request_id: RequestId) -> PromiseOrValue<bool> {
```

### Mutlisig contract guarantees and invariants

Guarantees:
 - Each request only gets executed after `num_confirmations` calls to `confirm` from different access keys. 

### Gotchas
 
User can delete access keys such that total number of different access keys will fall below `num_confirmations`, rendering contract locked.
 
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

Before deploying the contract, you need to collect all public keys that it will be initialized with.

Commands to deploy and initialize a 2 out of 3 multisig contract via `near repl`:

```javascript
const fs = require('fs');
const account = await near.account("illia");
const contractName = "multisig.illia";
const methodNames = ["add_request","delete_request","confirm"];
const newArgs = {"num_confirmations": 2};
const result = account.signAndSendTransaction(
    contractName,
    [
        nearAPI.transactions.createAccount(),
        nearAPI.transactions.transfer("100000000000000000000000000"),  
        nearAPI.transactions.addKey(
            nearAPI.utils.PublicKey.from("Eg2jtsiMrprn7zgKKUk79qM1hWhANsFyE6JSX4txLEuy"),
            nearAPI.transactions.functionCallAccessKey(contractName, methodNames, null)),
        nearAPI.transactions.addKey(
            nearAPI.utils.PublicKey.from("HghiythFFPjVXwc9BLNi8uqFmfQc1DWFrJQ4nE6ANo7R"),
            nearAPI.transactions.functionCallAccessKey(contractName, methodNames, null)),
        nearAPI.transactions.addKey(
            nearAPI.utils.PublicKey.from("2EfbwnQHPBWQKbNczLiVznFghh9qs716QT71zN6L1D95"),
            nearAPI.transactions.functionCallAccessKey(contractName, methodNames, null)),
        nearAPI.transactions.deployContract(fs.readFileSync("res/multisig.wasm")),
        nearAPI.transactions.functionCall("new", Buffer.from(JSON.stringify(newArgs)), 10000000000000, "0"),
    ]);
```

### Create request

To create request for transfer funds:
```bash
near call multisig.illia add_request '{"request": {"type": "Transfer", "receiver_id": "illia", "amount": "1000000000000000000000"}}' --accountId multisig.illia
```

Add another key to multisig:
```bash
near call multisig.illia add_request '{"request": {"type": "AddKey", "public_key": "<base58 of the key>"}}' --accountId multisig.illia
```

Change number of confirmations required to approve multisig:
```bash
near call multisig.illia add_request '{"request": {"type": "SetNumConfirmations", "num_confirmations": 2}}' --accountId multisig.illia
```

Returns the `request_id` of this request that can be used to confirm or see details.

As a side note, for this to work one of the keys from multisig should be available in your `~/.near-credentials/<network>/<multisig-name>.json` or use `--useLedgerKey` to sign with Ledger.

### Confirm request

To confirm a specific request:
```bash
near call multisig.illia confirm '{"request_id": 0}' --accountId multisig.illia
```

### View requests

To list all requests ids:
```bash
near view multisig.illia list_request_ids
```

To see information about specific request:
```bash
near view multisig.illia get_request '{"request_id": 0}'
```

To see confirmations for specific request:
```bash
near view multisig.illia get_confirmations '{"request_id": 0}'
```
