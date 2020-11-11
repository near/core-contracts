#!/usr/bin/env bash
set -ex

# Note: `staking-pool` has to be built before `staking-pool-factory`
for p in lockup multisig staking-pool staking-pool-factory voting whitelist
do
 (cd ${p} && ./build.sh)
done
