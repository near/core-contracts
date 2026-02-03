# Wrapped NEAR (wNEAR)

Source code for the wNEAR contract deployed at `wrap.near` on mainnet.

wNEAR is a [NEP-141](https://github.com/near/NEPs/blob/master/neps/nep-0141.md) fungible token that wraps native NEAR at a 1:1 ratio, enabling compatibility with bridges, wallets, and DeFi apps.

| | |
|--|--|
| **Contract** | `wrap.near` |
| **Symbol** | wNEAR |
| **Decimals** | 24 |
| **Standard** | [NEP-141](https://github.com/near/NEPs/blob/master/neps/nep-0141.md) |

## Methods

The contract maintains a ledger of wNEAR balances. Native NEAR only moves during wrap/unwrap—transfers just update the ledger.

### Wrap/Unwrap

| Method | Description |
|--------|-------------|
| `near_deposit()` | Wrap NEAR into wNEAR. Attach NEAR to the call. |
| `near_withdraw(amount)` | Unwrap wNEAR back to native NEAR. Requires 1 yoctoNEAR attached. |

### NEP-141 Token Methods

| Method | Description |
|--------|-------------|
| `ft_transfer(receiver_id, amount, memo)` | Transfer wNEAR to another account on the wrap.near contract |
| `ft_transfer_call(receiver_id, amount, memo, msg)` | Transfer with callback to receiver contract |
| `ft_balance_of(account_id)` | Get wNEAR balance of an account |
| `ft_total_supply()` | Get total wNEAR in circulation |

## Usage Examples

Examples using [NEAR CLI](https://github.com/near/near-cli-rs):

**Wrap NEAR to get wNEAR:**

```bash
near call wrap.near near_deposit --accountId your-account.near --deposit 10 --networkId mainnet
```

**Check wNEAR balance:**

```bash
near view wrap.near ft_balance_of '{"account_id": "your-account.near"}' --networkId mainnet
```

**Transfer wNEAR to another account:**

```bash
near call wrap.near ft_transfer \
  '{"receiver_id": "receiver.near", "amount": "5000000000000000000000000"}' \
  --accountId your-account.near \
  --depositYocto 1 \
  --networkId mainnet
```

**Unwrap wNEAR back to native NEAR:**

```bash
near call wrap.near near_withdraw \
  '{"amount": "5000000000000000000000000"}' \
  --accountId your-account.near \
  --depositYocto 1 \
  --networkId mainnet
```

**Register another account (so they can receive wNEAR):**

```bash
near call wrap.near storage_deposit \
  '{"account_id": "receiver.near"}' \
  --accountId your-account.near \
  --deposit 0.00125 \ 
  --networkId mainnet
```

## Notes

- Amounts are in yoctoNEAR: 1 NEAR = 10²⁴ yoctoNEAR
- `near_deposit()` auto-registers unregistered accounts by deducting ~0.00125 NEAR from the deposit
- Storage requirement: ~0.00125 NEAR per registered account

## License

MIT OR Apache-2.0
