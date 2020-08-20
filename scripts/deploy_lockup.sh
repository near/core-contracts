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


if [ -z "${LOCKUP_MASTER_ACCOUNT_ID}" ]; then
  echo "LOCKUP_MASTER_ACCOUNT_ID is required, e.g. \`export LOCKUP_MASTER_ACCOUNT_ID=lockup\`"
  exit 1
fi

echo "Using NODE_ENV=${NODE_ENV}"
echo "Using MASTER_ACCOUNT_ID=${MASTER_ACCOUNT_ID}"
echo "Using LOCKUP_MASTER_ACCOUNT_ID=${LOCKUP_MASTER_ACCOUNT_ID}"

# Verifying master account exist
RES=$(near state $LOCKUP_MASTER_ACCOUNT_ID | grep "amount" && echo "OK" || echo "BAD")
if [ "$RES" = "BAD" ]; then
  echo "Can't get state for ${LOCKUP_MASTER_ACCOUNT_ID}. Maybe the account doesn't exist."
  exit 1
fi

read -p "Enter account ID (prefix) to create: " ACCOUNT_PREFIX

PREFIX_RE=$(grep -qE '^([a-z0-9]+[-_])*[a-z0-9]+$' <<< "$ACCOUNT_PREFIX" && echo "OK" || echo "BAD")

if [ "$PREFIX_RE" = "OK" ]; then
  ACCOUNT_ID="$ACCOUNT_PREFIX.${LOCKUP_MASTER_ACCOUNT_ID}"
else
  echo "Invalid new account prefix."
  exit 1
fi

LOCKUP_ACCOUNT_ID=$ACCOUNT_ID

echo "Lockup account ID is $LOCKUP_ACCOUNT_ID"

if [ ${#LOCKUP_ACCOUNT_ID} -gt "64" ]; then
  echo "The legnth of the lockup account is longer than 64 characters"
  exit 1
fi

# Verifying the new account doesn't exist
RES=$(near state $ACCOUNT_ID | grep "amount" && echo "BAD" || echo "OK")
if [ "$RES" = "BAD" ]; then
  echo "The account ${ACCOUNT_ID} already exist."
  exit 1
fi

while true; do
  read -p "Enter OWNER_ACCOUNT_ID: " OWNER_ACCOUNT_ID

  # Verifying master account exist
  RES=$(near state $OWNER_ACCOUNT_ID | grep "amount" && echo "OK" || echo "BAD")
  if [ "$RES" = "BAD" ]; then
    echo "Can't get state for ${OWNER_ACCOUNT_ID}. Maybe the account doesn't exist."
  else
    echo "Using owner's account ID $OWNER_ACCOUNT_ID"
    break;
  fi
done

MINIMUM_BALANCE="35"
while true; do
  read -p "Enter the amount in NEAR tokens (not yocto) to deposit on lockup contract (min $MINIMUM_BALANCE): " LOCKUP_BALANCE
  if [ "$LOCKUP_BALANCE" -ge "$MINIMUM_BALANCE" ]; then
    echo "Going to deposit $LOCKUP_BALANCE tokens or ${LOCKUP_BALANCE}000000000000000000000000 yocto NEAR"
    break;
  else
    echo "The lockup balance has to be at least $MINIMUM_BALANCE NEAR tokens. Try again."
  fi
done

VOTE_ACCOUNT_ID="vote.${MASTER_ACCOUNT_ID}"
WHITELIST_ACCOUNT_ID="whitelist.${MASTER_ACCOUNT_ID}"

REPL=$(cat <<-END
await new Promise(resolve => setTimeout(resolve, 100));
const fs = require('fs');
const account = await near.account("$LOCKUP_MASTER_ACCOUNT_ID");
const contractName = "$ACCOUNT_ID";
const newArgs = {
    "owner_account_id": "$OWNER_ACCOUNT_ID",
    "lockup_duration": "259200000000000",
    "transfers_information": {
        "TransfersDisabled": {
            "transfer_poll_account_id": "$VOTE_ACCOUNT_ID"
        }
    },
    "release_duration": "2592000000000000",
    "staking_pool_whitelist_account_id": "$WHITELIST_ACCOUNT_ID",
};
await account.signAndSendTransaction(
    contractName,
    [
        nearAPI.transactions.createAccount(),
        nearAPI.transactions.transfer("${LOCKUP_BALANCE}000000000000000000000000"),
        nearAPI.transactions.deployContract(fs.readFileSync("../lockup/res/lockup_contract.wasm")),
        nearAPI.transactions.functionCall("new", Buffer.from(JSON.stringify(newArgs)), 10000000000000, "0"),
    ]);
END
)

#
#REPL=$(cat <<-END
#await new Promise(resolve => setTimeout(resolve, 100));
#const fs = require('fs');
#const account = await near.account("$LOCKUP_MASTER_ACCOUNT_ID");
#const contractName = "$ACCOUNT_ID";
#const newArgs = {
#    "owner_account_id": "$OWNER_ACCOUNT_ID",
#    "lockup_duration": "259200000000000",
#    "transfers_information": {
#        "TransfersEnabled": {
#            "transfers_timestamp": "1597600995135000000"
#        }
#    },
#    "release_duration": "2592000000000000",
#    "staking_pool_whitelist_account_id": "$WHITELIST_ACCOUNT_ID",
#};
#await account.signAndSendTransaction(
#    contractName,
#    [
#        nearAPI.transactions.createAccount(),
#        nearAPI.transactions.transfer("${LOCKUP_BALANCE}000000000000000000000000"),
#        nearAPI.transactions.deployContract(fs.readFileSync("../lockup/res/lockup_contract.wasm")),
#        nearAPI.transactions.functionCall("new", Buffer.from(JSON.stringify(newArgs)), 10000000000000, "0"),
#    ]);
#END
#)

echo $REPL | near repl
