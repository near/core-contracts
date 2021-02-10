# Lockup Factory

# Deployment & Usage

## TestNet

near dev-deploy --wasmFile=res/lockup_factory.wasm

# Initialize the factory.
near call lockup.nearnet new '{"whitelist_account_id":"whitelist.nearnet","foundation_account_id":"nearnet","master_account_id":"nearnet","lockup_master_account_id":"lockup.nearnet"}' --accountId lockup.nearnet     

# Create a new lockup with the given parameters.
near call lockup.nearnet create '{"owner_account_id":"lockup_owner.testnet","lockup_duration":"63036000000000000"}' --accountId funding_account.testnet --amount 50000

# Create a new lockup with the vesting schedule.
near call lockup.nearnet create '{"owner_account_id":"lockup_owner.testnet","lockup_duration":"31536000000000000","vesting_schedule": { "VestingSchedule": {"start_timestamp": "1535760000000000000", "cliff_timestamp": "1567296000000000000", "end_timestamp": "1661990400000000000"}}}' funding_account.testnet --amount 50000 --gas 110000000000000


