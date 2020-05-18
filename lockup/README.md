# Lockup / Vesting contract

This contract acts as an escrow that locks and holds owner's funds for the lockup period. The lockup period either starts
at the given timestamp or from the moment transfers are enabled by voting.
If transfers are not enabled yet, the contract keeps the account ID of the transfer poll contract.
When the transfer poll is resolved, it returns the timestamp when it was resolved and it's used as the beginning of the
lockup period.

## Vesting

The contract can also contain a vesting schedule.
In this case, this contract serves as a vesting agreement between the NEAR Foundation (Foundation) and an employee (owner of contract).
Vesting schedule is described by 3 timestamps in nanoseconds:
- `start_timestamp` - The timestamp in nanosecond when the vesting starts. E.g. the start date of employment.
- `cliff_timestamp` - The timestamp in nanosecond when the first part of lockup tokens becomes vested.
 The remaining tokens will vest continuously until they are fully vested.
 Example: a 1 year of employment at which moment the 1/4 of tokens become vested.
- `end_timestamp` - The timestamp in nanosecond when the vesting ends.

In addition to the lockup period that starts from the moment the transfers are enabled, vesting schedule also locks
all funds until `cliff_timestamp`.
Once the `cliff_timestamp` passed, the funds are vested linearly from the `start_timestamp` to the `end_timestamp`.

If the employee (owner) is terminated before the cliff all tokens are refunded to the Foundation.
Otherwise the remaining unvested funds are refunded.

## Staking

NEAR is the proof of stake network. The owner of the lockup contract might hold large percentage of the network tokens.
The owner may want to stake these tokens (including locked tokens) to help secure the network and also earn staking rewards that are distributed to network validator.
The contract doesn't allow to directly stake from this account, so the owner can delegate tokens to a staking pool contract (see https://github.com/near/initial-contracts/tree/master/staking-pool).

The owner can choose the staking pool where to delegate tokens.
The staking pool contract and the account has to be approved and whitelisted by the foundation, to prevent lockup tokens from being lost, locked or stolen.
This staking pool must be an approved account, which is validated by a whitelisting contract.
Once the staking pool holds tokens, the owner of the staking pool is able to use them to vote on the network governence issues, such as enabling transfers.
So it's important for the owner to pick the staking pool that fits the best.

Formally the contract includes:

1. Lockup information.
2. A whitelisted staking pool's account ID.
3. A transfer poll account ID.
4. Employee (Owner)'s public key and optionally their staking public key.
5. Optionally the public key of the Foundation.


### Lockup Information
This includes:
1. Amount of tokens to lockup.
2. Length of lockup.
3. Optional timestamp of when transfers started.
4. Optional vesting information

#### Vesting Information
Either:
1. Vesting Schedule:
  A) Start timestamp time in nano-seconds.
  B) Cliff timestamp
  C) End timestamp

2. Termination Status:
  A)
  B)

