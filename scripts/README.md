# Setup scripts

## Rebuild all contracts

```bash
./build_all.sh
```

## Deploy core contracts using master account

### Set master account

```bash
export MASTER_ACCOUNT_ID=near
```

### Set network environment

```bash
export MASTER_ACCOUNT_ID=testnet
```

### Deploy

This will deploy the following contracts:

- Voting contract at `vote.<MASTER_ACCOUNT_ID>`
- Whitelist contract at `whitelist.<MASTER_ACCOUNT_ID>`
- Staking pool factory contract at `pool.<MASTER_ACCOUNT_ID>`

It will whitelist the staking pool factory account.

It requires `80` NEAR tokens.

```bash
./deploy_core.sh
```

## Notes

For rebuilding contracts, make sure you have `rust` with `wasm32` target installed.

For deploying, you need to have `near-shell` installed and be logged in with the master account ID.
