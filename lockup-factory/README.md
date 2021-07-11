# Lockup Factory Contract

This contract deploys lockup contracts. 
It allows any user to create and fund the lockup contract.
The lockup factory contract packages the binary of the 
<a href="https://github.com/near/core-contracts/tree/master/lockup">lockup 
contract</a> within its own binary.

To create a new lockup contract a user should issue a transaction and 
attach the required minimum deposit. The entire deposit will be transferred to 
the newly created lockup contract including to cover the storage.

The benefits: 
1. Lockups can be funded from any account.
2. No need to have access to the foundation keys to create lockup.
3. Auto-generates the lockup from the owner account.
4. Refund deposit on errors.



# Deployment & Usage

## TestNet

near dev-deploy --wasmFile=res/lockup_factory.wasm

# Initialize the factory.
near call lockup.nearnet new '{"whitelist_account_id":"whitelist.nearnet","foundation_account_id":"nearnet","master_account_id":"nearnet","lockup_master_account_id":"lockup.nearnet"}' --accountId lockup.nearnet     

# Create a new lockup with the given parameters.
near call lockup.nearnet create '{"owner_account_id":"lockup_owner.testnet","lockup_duration":"63036000000000000"}' --accountId funding_account.testnet --amount 50000

# Create a new lockup with the vesting schedule.
near call lockup.nearnet create '{"owner_account_id":"lockup_owner.testnet","lockup_duration":"31536000000000000","vesting_schedule": { "VestingSchedule": {"start_timestamp": "1535760000000000000", "cliff_timestamp": "1567296000000000000", "end_timestamp": "1661990400000000000"}}}' --accountId funding_account.testnet --amount 50000 --gas 110000000000000


