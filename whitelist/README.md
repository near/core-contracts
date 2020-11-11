# Whitelist contract for staking pools

The purpose of this contract is to maintain the whitelist of the staking pool contracts account IDs that are approved
by NEAR Foundation.

In order for the lockup contracts to be able delegate to a staking pool, the staking pool should faithfully implement the spec.
The staking pool should guarantee that the delegated tokens can not be lost or locked, such as the lockup contract should be
able to recover delegated tokens back to the lockup from a staking pool. In order to enforce this, only approved (whitelisted)
accounts of staking pool contracts can receive delegated tokens from lockup contracts.

If NEAR Foundation has to approve every single staking pool account it might lead to a bottleneck and centralization
To address this NEAR Foundation can whitelist the account IDs of staking pool factory contracts.

The whitelisted staking pool factory contract will be able to whitelist accounts of staking pool contracts.
A factory contract creates and initializes a staking pool contract in a secure and permissionless way.
This allows anyone on the network to be able to create a staking pool contract for themselves without needing approval from the NEAR
Foundation. This is important to maintain the decentralization of the decision making and network governance.

To be able to address mistakes, NEAR Foundation has the ability to remove staking pools and staking pool factories from the whitelists.

## Requirements and guarantees

- The account of the whitelist contract should not contain any access keys, to avoid it from being deleted.
- If the account runs out of tokens for storage, any account can fund it. In theory the gas rebates may cover the storage in the long term.
- `is_whitelisted` call doesn't panic, unless it's given insufficient amount of gas or the invalid account ID.
- The contract maintains two separate whitelists, one for staking pools and one for factories.

## API

The methods are split into Getters (view methods), the method that can be called by both an approved factory and the foundation,
and methods that can only be called by the foundation.

## Changelog

### `0.2.0`

- Internally updated to use `LockupSet` instead of `UnorderedSet`.

```rust
/// Initializes the contract with the given NEAR foundation account ID.
#[init]
pub fn new(foundation_account_id: AccountId) -> Self;

/***********/
/* Getters */
/***********/

/// Returns `true` if the given staking pool account ID is whitelisted.
pub fn is_whitelisted(&self, staking_pool_account_id: AccountId) -> bool;

/// Returns `true` if the given factory contract account ID is whitelisted.
pub fn is_factory_whitelisted(&self, factory_account_id: AccountId) -> bool;

/************************/
/* Factory + Foundation */
/************************/

/// Adds the given staking pool account ID to the whitelist.
/// Returns `true` if the staking pool was not in the whitelist before, `false` otherwise.
/// This method can be called either by the NEAR foundation or by a whitelisted factory.
pub fn add_staking_pool(&mut self, staking_pool_account_id: AccountId) -> bool;

/**************/
/* Foundation */
/**************/

/// Removes the given staking pool account ID from the whitelist.
/// Returns `true` if the staking pool was present in the whitelist before, `false` otherwise.
/// This method can only be called by the NEAR foundation.
pub fn remove_staking_pool(&mut self, staking_pool_account_id: AccountId) -> bool;

/// Adds the given staking pool factory contract account ID to the factory whitelist.
/// Returns `true` if the factory was not in the whitelist before, `false` otherwise.
/// This method can only be called by the NEAR foundation.
pub fn add_factory(&mut self, factory_account_id: AccountId) -> bool;

/// Removes the given staking pool factory account ID from the factory whitelist.
/// Returns `true` if the factory was present in the whitelist before, `false` otherwise.
/// This method can only be called by the NEAR foundation.
pub fn remove_factory(&mut self, factory_account_id: AccountId) -> bool;
```
