# Lockup / Vesting contract

This contract serves as a vesting agreement between the NEAR Foundation (Foundation) and an employee (owner of contract).  The contract is created upon employment, where an amount of NEAR tokens (tokens) are locked starting at a nano-second timestamp.  This remains fully locked until a cliff timestamp (e.g. a quarter of a year), which unlocks a fraction (e.g. one fourth).  If the employee (owner) is terminated before the cliff all tokens are refunded to the Foundation.  Otherwise it is prorated starting from the cliff timestamp.

The owner can also specify which staking pool contract they would like to store (stake) their tokens in, which increases gains a portion of rewards from the pool over time.  This staking pool must be an approved account, which is validated by a whitelisting contract.


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

