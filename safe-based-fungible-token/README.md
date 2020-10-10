# Safe Based Fungible token

Expected storage per account is `77 bytes = 40 (for a k/v record) + 1 (for a prefix) + 20 (for a key) + 16 (for a balance)`
Expected storage per safe `77 bytes = 40 (for k/v) + 1 (for a prefix) + 8 (for key) + 28 (for a value)` but it's temporary for the duration of the transaction.

The `77` bytes requires state stake of `0.0077 NEAR = 77 bytes * 0.0001 NEAR/byte`.

Gas is `1T gas * 100M yoctoNEAR/gas = 0.0001 NEAR`. So to cover 69 bytes, you need at least `77 Tgas` transferred by the contract.

## TODO

- [ ] Write README
- [ ] Add unit tests
- [ ] Decide on storage usage
- [ ] Add integration tests
