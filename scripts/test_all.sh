#!/usr/bin/env bash
set -ex -o pipefail

# Note: `staking-pool` has to be built before `staking-pool-factory`
jq -c '.[]' scripts/contracts.json | while read i; do
  CONTRACT_DIR=$(echo $i | jq -r '.contract_dir')
  (cd $CONTRACT_DIR && RUSTFLAGS='-D warnings' cargo test)
done
