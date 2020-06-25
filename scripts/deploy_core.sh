#!/bin/bash
set -e

if [ -z "${NODE_ENV}" ]; then
  echo "NODE_ENV is required, e.g. \`export NODE_ENV=testnet\`"
  exit 1
fi

if [ -z "${MASTER_ACCOUNT_ID}" ]; then
  echo "MASTER_ACCOUNT_ID is required, e.g. \`export MASTER_ACCOUNT_ID=near\`"
  exit 1
fi

echo "Using NODE_ENV=${NODE_ENV}"
echo "Using MASTER_ACCOUNT_ID=${MASTER_ACCOUNT_ID}"

# Verifying master account exist
AMOUNT=$(near state $MASTER_ACCOUNT_ID | grep "amount")
if [ -z "$AMOUNT" ]; then
  echo "Can't get state for ${MASTER_ACCOUNT_ID}. Maybe the account doesn't exist."
  exit 1
fi

pushd deploy

./deploy_voting.sh
./deploy_whitelist.sh
./deploy_staking_pool_factory.sh

popd
