# Generic Factory

Factory contract that can deploy contract as a sub-accounts with given parameters.

Methods:
 - `store(code: <bytes>)` - only owner, update code inside the factory.
 - `create(name: AccountId, hash: Base58CryptoHash, args: BaseU8Vec)` - creates new contract and calls `new` with given args.

# Deployment

## TestNet

### Deploy factory

```
> near dev-deploy --wasmFile=res/generic_factory.wasm
```

### Store contract 

```javascript
const accountId = "<your account>";
const contractName = "<contract id from dev-deploy>";
const fs = require('fs');
const account = await near.account(accountId);
const code = fs.readFileSync("../multisig/res/multisig.wasm");
account.signAndSendTransaction(
    contractName,
    [
        nearAPI.transactions.functionCall("store", code, 20000000000000, "10"),
    ]);
```

### Create new contract from factory

```
> export ARGS = "" | base64
> near call <contract id> create "{\"name\": \"test\", \"hash\": \"<hash from prev call>\", \"args\": \"ARGS\",\"access_keys\": [\"<public keys>\"]}" --amount 1
```
