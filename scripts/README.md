# Setup scripts

## Rebuild all contracts

```bash
./build_all_docker.sh
```

## Deploy core contracts using master account

### Set master account

```bash
export MASTER_ACCOUNT_ID=near
```

### Set network environment

```bash
export NEAR_ENV=testnet
```

### Deploy

This will deploy the following contracts:

- Voting contract at `vote.<MASTER_ACCOUNT_ID>` with `15` NEAR tokens
- Whitelist contract at `whitelist.<MASTER_ACCOUNT_ID>` with `15` NEAR tokens
- Staking pool factory contract at `pool.<MASTER_ACCOUNT_ID>` with `50` NEAR tokens

It will whitelist the staking pool factory account.

It requires total `80` NEAR tokens + gas fees.

```bash
./deploy_core.sh
```

## Deploying lockup contract

NOTE: This flow is mostly for testnet and is not recommended for production use.

### Set lockup root account

This account will be used as a suffix to deploy lockup contracts.
Also this account will fund the newly created lockup contracts.

```bash
export LOCKUP_MASTER_ACCOUNT_ID=lockup.near
```

### Deploying

To deploy a lockup call the script. It has interactive interface to provide details.

```bash
./deploy_lockup.sh
```

Once the amount (in NEAR) is provided, the lockup contract will be deployed.

## Notes

For rebuilding contracts, make sure you have `rust` with `wasm32` target installed.

For deploying, you need to have `near-shell` installed and be logged in with the master account ID.
