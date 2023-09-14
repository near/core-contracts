# State Manipulation contract

This contract has been designed to put key value pairs into storage with `replace` and clear key/value pairs with `clean` from an account's storage.

Deploy this contract into the account that already has another contract deployed to it.
This contract on call `clean` will remove any items of the state specified (keys should be specified in base64). When compiled with `replace` feature, `replace` method can be called with an array of key/value tuple pairs to insert into state.

## Parameters

JSON format for `clean`:

```json
{"keys":["<base64 encoded key byte string>", "...", "..."]}
```

JSON format for `replace`:
```json
{"entries":[["<base64 key byte string>", "<base64 value byte string>"], ["...", "..."]]}
```
## With CLI

Usage example to put and remove only the "STATE" item using [near-cli](https://github.com/near/near-cli-rs):

```bash
# Build the contracts will all feature combinations
./build.sh

# Deploy built code on chain
near-cli add contract-code network testnet account nesdie.testnet contract-file ./res/state_manipulation.wasm no-initialize sign-with-keychain

# Add state item for "STATE" key
near-cli execute change-method network testnet contract nesdie.testnet call replace '{"entries":[["U1RBVEU=", "dGVzdA=="]]}' --prepaid-gas '100.000 TeraGas' --attached-deposit '0 NEAR' signer nesdie.testnet sign-with-keychain

# View Added state item
near-cli view contract-state network testnet account nesdie.testnet at-final-block

# Clear added state item
near-cli execute change-method network testnet contract nesdie.testnet call clean '{"keys":["U1RBVEU="]}' --prepaid-gas '100.000 TeraGas' --attached-deposit '0 NEAR' signer nesdie.testnet sign-with-keychain

# View that item was removed
near-cli view contract-state network testnet account nesdie.testnet at-final-block
```

## Features
`clean`: Enables `clean` method to remove keys
`replace`: Enables `replace` method to add key/value pairs to storage