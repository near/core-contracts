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

read -p "Enter account ID (prefix) to create: " ACCOUNT_PREFIX

PREFIX_RE=$(grep -qE '^([a-z0-9]+[-_])*[a-z0-9]+$' <<< "$ACCOUNT_PREFIX" && echo "OK" || echo "BAD")

if [ "$PREFIX_RE" = "OK" ]; then
  ACCOUNT_ID="$ACCOUNT_PREFIX.${MASTER_ACCOUNT_ID}"
else
  echo "Invalid new account prefix."
  exit 1
fi

LOCKUP_ACCOUNT_ID="lockup.$ACCOUNT_ID"

echo "Multisig account ID is $ACCOUNT_ID"
echo "Lockup account ID is $LOCKUP_ACCOUNT_ID"

LOCKUP_ACCOUNT_ID_LEN=${#LOCKUP_ACCOUNT_ID}

if [ ${#LOCKUP_ACCOUNT_ID} -gt "64" ]; then
  echo "The legnth of the lockup account is longer than 64 characters"
  exit 1
fi

PUBLIC_KEYS=()

for i in {1..3}; do
  while true; do
    read -p "New account multisig public key in base58 format ($i/3): " KEY
    REPL=$(echo "nearAPI.utils.key_pair.PublicKey.fromString('$KEY').data.length == 32")
    RES=$(echo "$REPL" | near repl | grep -q "true" && echo "OK" || echo "BAD")
    if [ "$RES" = "OK" ]; then
      break;
    else
      echo "Invalid public key. Try again."
    fi
  done
  PUBLIC_KEYS+=( $KEY )
done

read -p "Enter the amount of yocto-NEAR tokens to deposit on lockup contract: " LOCKUP_BALANCE



#
#if
#
#echo "Deploying staking pool factory contract to $ACCOUNT_ID with 50 NEAR"
#
#
#REPL=$(cat <<-END
#const fs = require('fs');
#const account = await near.account("$MASTER_ACCOUNT_ID");
#const contractName = "$ACCOUNT_ID";
#const newArgs = {staking_pool_whitelist_account_id: "$WHITELIST_ACCOUNT_ID"};
#await account.signAndSendTransaction(
#    contractName,
#    [
#        nearAPI.transactions.createAccount(),
#        nearAPI.transactions.transfer("50000000000000000000000000"),
#        nearAPI.transactions.deployContract(fs.readFileSync("../../staking-pool-factory/res/staking_pool_factory.wasm")),
#        nearAPI.transactions.functionCall("new", Buffer.from(JSON.stringify(newArgs)), 10000000000000, "0"),
#    ]);
#END
#)
#
#echo $REPL | near repl
#
#echo "Whetelisting staking pool factory $ACCOUNT_ID on whitelist contract $WHITELIST_ACCOUNT_ID"
#
#REPL=$(cat <<-END
#const account = await near.account("$MASTER_ACCOUNT_ID");
#const contractName = "$WHITELIST_ACCOUNT_ID";
#const args = {factory_account_id: "$ACCOUNT_ID"};
#await account.signAndSendTransaction(
#    contractName,
#    [
#        nearAPI.transactions.functionCall("add_factory", Buffer.from(JSON.stringify(args)), 10000000000000, "0"),
#    ]);
#END
#)
#
#echo $REPL | near repl
