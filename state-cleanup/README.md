# State Cleanup contract

This contract has been designed to clear account without deleting it.

Deploy this contract into the account that already has another contract deployed to it.
This contract on call `clean` will remove any items of the state specified.

Usage example:

```bash
near deploy l.testmewell.testnet --wasmFile=res/state_cleanup.wasm
near call l.testmewell.testnet clean '{"keys": ["STATE"]}' --accountId testmewell.testnet
```

To check for keys and check that everything was indeed removed use this command:
```
near view-state l.testmewell.testnet --finality final
```
