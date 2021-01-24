# Multisig Factory

Allows to create new [Multisig contracts](../multisig) just by sending a transactions with the required configuration and funds.
E.g. Removes need for using `near repl` and having wasm file available.

# Deployment & Usage

## TestNet

Deploy to new developer account on TestNet:

```
near dev-deploy --wasmFile=res/multisig_factory.wasm
```

Setup variable for the contract to use commands below easier:

```
# bash
CONTRACT_ID="dev-1608694678554-8567049"

# fish
set CONTRACT_ID "dev-1608694678554-8567049"
```

Create a new multisig with the given parameters and attached amount (50N) passed to multisig contract:

```
near call $CONTRACT_ID create '{"name": "test", "members": [{"account_id": "illia"}, {"account_id": "testmewell.testnet"}, {"public_key": "ed25519:Eg2jtsiMrprn7zgKKUk79qM1hWhANsFyE6JSX4txLEuy"}], "num_confirmations": 1}'  --accountId $CONTRACT_ID --amount 50 --gas 100000000000000
```
