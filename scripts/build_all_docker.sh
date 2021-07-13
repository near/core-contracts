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
jq -c '.[]' scripts/contracts.json | while read i; do
  CONTRACT_DIR=$(echo $i | jq -r '.contract_dir')
  CONTRACT_NAME=$(echo $i | jq -r '.contract_name')
	(./scripts/build_docker.sh $CONTRACT_DIR $CONTRACT_NAME)
done

if [ $CHECK == 1 ] && [ ! -z "$(git diff --exit-code)" ] ; then
	echo "Repository is dirty, please make sure you have committed all contract wasm files"
	exit 1
fi
