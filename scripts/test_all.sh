#!/usr/bin/env bash
set -ex

# Note: `staking-pool` has to be built before `staking-pool-factory`
while read -r contract
do
 	(cd ${contract} && RUSTFLAGS='-D warnings' cargo test)
done < scripts/CONTRACTS
