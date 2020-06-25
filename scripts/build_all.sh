#!/usr/bin/env bash
set -ex

for p in lockup multisig staking-pool staking-pool-factory voting whitelist
do
  pushd ../${p}
  ./build.sh
  popd
done
