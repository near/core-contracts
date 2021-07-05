#!/usr/bin/env bash
set -ex

CHECK=0

# Loop through arguments and process them
for arg in "$@"
do
    case $arg in
        -c|--check)
        CHECK=1
        shift 
        ;;
    esac
done

# Note: `staking-pool` has to be built before `staking-pool-factory`
for p in lockup multisig staking-pool staking-pool-factory voting whitelist
do
 (cd ${p} && ./build.sh)
done

if [ $CHECK == 1 ] && [ "$(git diff --exit-code)" != 0 ] ; then
	echo "Repository is dirty, please make sure you have committed all contract wasm files"
	exit 1
fi
