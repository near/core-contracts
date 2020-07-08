#!/bin/bash
set -e

WHITELIST_ACCOUNT_ID="whitelist.${MASTER_ACCOUNT_ID}"
ACCOUNT_ID="pool.${MASTER_ACCOUNT_ID}"

echo "Deploying staking pool factory contract to $ACCOUNT_ID with 50 NEAR"


REPL=$(cat <<-END
const fs = require('fs');
const account = await near.account("$MASTER_ACCOUNT_ID");
const contractName = "$ACCOUNT_ID";
const newArgs = {staking_pool_whitelist_account_id: "$WHITELIST_ACCOUNT_ID"};
await account.signAndSendTransaction(
    contractName,
    [
        nearAPI.transactions.createAccount(),
        nearAPI.transactions.transfer("50000000000000000000000000"),
        nearAPI.transactions.deployContract(fs.readFileSync("../../staking-pool-factory/res/staking_pool_factory.wasm")),
        nearAPI.transactions.functionCall("new", Buffer.from(JSON.stringify(newArgs)), 10000000000000, "0"),
    ]);
END
)

echo $REPL | near repl

echo "Whitelisting staking pool factory $ACCOUNT_ID on whitelist contract $WHITELIST_ACCOUNT_ID"

REPL=$(cat <<-END
const account = await near.account("$MASTER_ACCOUNT_ID");
const contractName = "$WHITELIST_ACCOUNT_ID";
const args = {factory_account_id: "$ACCOUNT_ID"};
await account.signAndSendTransaction(
    contractName,
    [
        nearAPI.transactions.functionCall("add_factory", Buffer.from(JSON.stringify(args)), 10000000000000, "0"),
    ]);
END
)

echo $REPL | near repl
