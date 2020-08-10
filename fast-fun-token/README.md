# Small and fast fungible tokens


Has 3 methods:

```rust
/// Initializes the token contract with the total supply given to the owner.
/// Arguments (64 bytes):
/// - 0..32 - `sha256` of the owner address.
/// - 32..64 - U256 of the total supply in LE bytes
pub fn init();

/// Transfer the amount from the `sha256(predecessor_account_id)` to the new receiver address.
/// Arguments (64 bytes):
/// - 0..32 - `sha256` of the receiver address.
/// - 32..64 - U256 is transfer amount in LE bytes
pub fn transfer();

/// Returns the balance of the given address.
/// Arguments (64 bytes):
/// - 0..32 - `sha256` of the address to check the balance.
pub fn get_balance();
```
