# State Cleanup contract

This contract has been designed to clear account without deleting it.

Deploy this contract into the account that already has another contract deployed to it.
This contract on call `clean` will remove any items of the state specified (keys should be specified in base64).

Usage example to remove "STATE" item:

```bash
near deploy l.testmewell.testnet --wasmFile=res/state_cleanup.wasm
near call l.testmewell.testnet clean '{"keys": ["U1RBVEU="]}' --accountId testmewell.testnet
```

To check which keys to remove and later check that everything has been removed use this command:
```
near view-state l.testmewell.testnet --finality final
```
