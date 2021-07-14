# Core contracts

- [Lockup / Vesting contract](./lockup/)
- [Lockup Factory](./lockup-factory/)
- [Multisig contract](./multisig/)
- [Staking Pool / Delegation contract](./staking-pool/)
- [Staking Pool Factory](./staking-pool-factory/)
- [Voting Contract](./voting/)
- [Whitelist Contract](./whitelist/)

## Building and deploying

See [scripts](./scripts/) folder for details.

## Initializing Contracts with near-shell

When setting up the contract creating the contract account, deploying the binary, and initializing the state must all be done as an atomic step.  For example, in our tests for the lockup contract we initialize it like this:

```rust
pub fn init_lockup(
        &self,
        runtime: &mut RuntimeStandalone,
        args: &InitLockupArgs,
        amount: Balance,
    ) -> TxResult {
        let tx = self
            .new_tx(runtime, LOCKUP_ACCOUNT_ID.into())
            .create_account()
            .transfer(ntoy(35) + amount)
            .deploy_contract(LOCKUP_WASM_BYTES.to_vec())
            .function_call(
                "new".into(),
                serde_json::to_vec(args).unwrap(),
                200000000000000,
                0,
            )
            .sign(&self.signer);
        let res = runtime.resolve_tx(tx).unwrap();
        runtime.process_all().unwrap();
        outcome_into_result(res)
    }
```


To do this with near shell, first add a script like `deploy.js`:

```js
const fs = require('fs');
const account = await near.account("foundation");
const contractName = "lockup-owner-id";
const newArgs = {
  "lockup_duration": "31536000000000000",
  "lockup_start_information": {
    "TransfersDisabled": {
        "transfer_poll_account_id": "transfers-poll"
    }
  },
  "vesting_schedule": {
    "start_timestamp": "1535760000000000000",
    "cliff_timestamp": "1567296000000000000",
    "end_timestamp": "1661990400000000000"
  },
  "staking_pool_whitelist_account_id": "staking-pool-whitelist",
  "initial_owners_main_public_key": "KuTCtARNzxZQ3YvXDeLjx83FDqxv2SdQTSbiq876zR7",
  "foundation_account_id": "near"
}
const result = account.signAndSendTransaction(
    contractName,
    [
        nearAPI.transactions.createAccount(),
        nearAPI.transactions.transfer("100000000000000000000000000"),
        nearAPI.transactions.deployContract(fs.readFileSync("res/lockup_contract.wasm")),
        nearAPI.transactions.functionCall("new", Buffer.from(JSON.stringify(newArgs)), 100000000000000, "0"),
    ]);
```

Then use the `near repl` command. Once at the command prompt, load the script:

```js
> .load deploy.js
```

Note: `nearAPI` and `near` are both preloaded to the repl's context.
