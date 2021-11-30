# Basic Multisig contract

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
/// Lowest level action that can be performed by the multisig contract.
pub enum MultiSigRequestAction {
    /// Transfers given amount to receiver.
    Transfer {
        amount: U128,
    },
    /// Create a new account.
    CreateAccount,
    /// Deploys contract to receiver's account. Can upgrade given contract as well.
    DeployContract { code: Base64VecU8 },
    /// Adds key, either new key for multisig or full access key to another account.
    AddKey {
        public_key: Base58PublicKey,
        #[serde(skip_serializing_if = "Option::is_none")]
        permission: Option<FunctionCallPermission>,
    },
    /// Deletes key, either one of the keys from multisig or key from another account.
    DeleteKey {
        public_key: Base58PublicKey,
    },
    /// Call function on behalf of this contract.
    FunctionCall {
        method_name: String,
        args: Base64VecU8,
        deposit: U128,
        gas: U64,
    },
    /// Sets number of confirmations required to authorize requests.
    /// Can not be bundled with any other actions or transactions.
    SetNumConfirmations {
        num_confirmations: u32,
    },
    /// Sets number of active requests (unconfirmed requests) per access key
    /// Default is 12 unconfirmed requests at a time
    /// The REQUEST_COOLDOWN for requests is 15min
    /// Worst gas attack a malicious keyholder could do is 12 requests every 15min
    SetActiveRequestsLimit {
        active_requests_limit: u32,
    },
}

/// Permission for an access key, scoped to receiving account and method names with allowance to add when key is added to accoount
pub struct FunctionCallPermission {
    allowance: Option<U128>,
    receiver_id: AccountId,
    method_names: Vec<String>,
}

// The request the user makes specifying the receiving account and actions they want to execute (1 tx)
pub struct MultiSigRequest {
    receiver_id: AccountId,
    actions: Vec<MultiSigRequestAction>,
}

// An internal request wrapped with the signer_pk and added timestamp to determine num_requests_pk and prevent against malicious key holder gas attacks
pub struct MultiSigRequestWithSigner {
    request: MultiSigRequest,
    signer_pk: PublicKey,
    added_timestamp: u64,
}
```

### Methods

```rust
/// Add request for multisig.
pub fn add_request(&mut self, request: MultiSigRequest) -> RequestId {

/// Add request for multisig and confirm right away with the key that is adding the request.
pub fn add_request_and_confirm(&mut self, request: MultiSigRequest) -> RequestId {

/// Remove given request and associated confirmations.
pub fn delete_request(&mut self, request_id: RequestId) {

/// Confirm given request with given signing key.
/// If with this, there has been enough confirmation, a promise with request will be scheduled.
pub fn confirm(&mut self, request_id: RequestId) -> PromiseOrValue<bool> {
```

### View Methods
```rust
pub fn get_request(&self, request_id: RequestId) -> MultiSigRequest
pub fn get_num_requests_pk(&self, public_key: Base58PublicKey) -> u32
pub fn list_request_ids(&self) -> Vec<RequestId>
pub fn get_confirmations(&self, request_id: RequestId) -> Vec<Base58PublicKey>
pub fn get_num_confirmations(&self) -> u32
pub fn get_request_nonce(&self) -> u32
```

### State machine

Per each request, multisig maintains next state machine:
 - `add_request` adds new request with empty list of confirmations.
 - `add_request_and_confirm` adds new request with 1 confirmation from the adding key.
 - `delete_request` deletes request and ends state machine.
 - `confirm` either adds new confirmation to list of confirmations or if there is more than `num_confirmations` confirmations with given call - switches to execution of request. `confirm` fails if request is already has been confirmed and already is executing which is determined if `confirmations` contain given `request_id`.
 - each step of execution, schedules a promise of given set of actions on `receiver_id` and puts a callback.
 - when callback executes, it checks if promise executed successfully: if no - stops executing the request and return failure. If yes - execute next transaction in the request if present.
 - when all transactions are executed, remove request from `requests` and with that finish the execution of the request.   

### Gotchas
 
User can delete access keys on the multisig such that total number of different access keys will fall below `num_confirmations`, rendering contract locked.
This is due to not having a way to query blockchain for current number of access keys on the account. See discussion here - https://github.com/nearprotocol/NEPs/issues/79.
 
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
near call multisig.illia add_request '{"request": {"receiver_id": "illia", "actions": [{"type": "Transfer", "amount": "1000000000000000000000"}]}}' --accountId multisig.illia
```

Add another key to multisig:
```bash
near call multisig.illia add_request '{"request": {"receiver_id": "multisig.illia", "actions": [{"type": "AddKey", "public_key": "<base58 of the key>"}]}}' --accountId multisig.illia
```

Change number of confirmations required to approve multisig:
```bash
near call multisig.illia add_request '{"request": {"receiver_id": "multisig.illia", "actions": [{"type": "SetNumConfirmations", "num_confirmations": 2}]}}' --accountId multisig.illia
```

Returns the `request_id` of this request that can be used to confirm or see details.

As a side note, for this to work one of the keys from multisig should be available in your `~/.near-credentials/<network>/<multisig-name>.json` or use `--useLedgerKey` to sign with Ledger.

You can also create a way more complex call that chains calling multiple different contracts:

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

Total confirmations required for any request:
```
near view multisig.illia get_num_confirmations
```

### Upgrade given multisig with new code

Create a request that deploys new contract code on the given account.
Be careful about data and requiring migrations (contract updates should include data migrations going forward). 

```javascript
const fs = require('fs');
const account = await near.account("multisig.illia");
const contractName = "multisig.illia";
const requestArgs = {"request": [
    {"receiver_id": "multisig.illia", "actions": [{"type": "DeployContract", "code": fs.readFileSync("res/multisig.wasm")}]}
]};
const result = account.signAndSendTransaction(
    contractName,
    [
        nearAPI.transactions.functionCall("add_request", Buffer.from(JSON.stringify(requestArgs)), 10000000000000, "0"),
    ]);
```

After this, still will need to confirm this with `num_confirmations` you have setup for given contract.

### Common commands for multisig

__Create an account__

```
near call illia.near add_request '{"request": {"receiver_id": "new_account.near", "actions": [{"type", "CreateAccount"}, {"type": "Transfer", "amount": "1000000000000000000000"}, {"type": "AddKey", "public_key": "<public key for the new account>"}]}}' --accountId illia.near
near call illia.near confirm '{"request_id": <request number from previous line>}'
```
