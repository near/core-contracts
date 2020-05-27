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

Commands to deploy and initialize a 2 out of 3 multisig contract via `near repl`:

```javascript
const tx = nearlib.transaction.createTransaction(
[
    nearlib.transactions.createAccount(),
    nearlib.transactions.transfer(10),  
    nearlib.transactions.addKey(`<1st_public_key>`),
    nearlib.transactions.addKey(`<2nd_public_key>`),
    nearlib.transactions.addKey(`<3nd_public_key>`),
    nearlib.transactions.deploy(),
    nearlib.transactions.functionCall("new", {"num_confirmations": 2}, 0, 10000000000000000),
]);
```
