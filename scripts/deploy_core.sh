#!/bin/bash
set -e

if [ -z "${NEAR_ENV}" ]; then
  echo "NEAR_ENV is required, e.g. \`export NEAR_ENV=testnet\`"
  exit 1
fi

if [ -z "${MASTER_ACCOUNT_ID}" ]; then
  echo "MASTER_ACCOUNT_ID is required, e.g. \`export MASTER_ACCOUNT_ID=near\`"
  exit 1
fi

if [ -z "${FOUNDATION_ACCOUNT_ID}"]; then
  echo "FOUNDATION_ACCOUNT_ID is required, e.g. \`export FOUNDATION_ACCOUNT_ID=foundation.near\`"
fi

echo "Using NEAR_ENV=${NEAR_ENV}"
echo "Using MASTER_ACCOUNT_ID=${MASTER_ACCOUNT_ID}"
echo "Using FOUNDATION_ACCOUNT_ID=${FOUNDATION_ACCOUNT_ID}"

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
