#!/bin/bash
set -e

ACCOUNT_ID="vote.${MASTER_ACCOUNT_ID}"

echo "Deploying voting contract to $ACCOUNT_ID with 15 NEAR"


REPL=$(cat <<-END
await new Promise(resolve => setTimeout(resolve, 100));
const fs = require('fs');
const account = await near.account("$MASTER_ACCOUNT_ID");
const contractName = "$ACCOUNT_ID";
const newArgs = {};
await account.signAndSendTransaction(
    contractName,
    [
        nearAPI.transactions.createAccount(),
        nearAPI.transactions.transfer("15000000000000000000000000"),
        nearAPI.transactions.deployContract(fs.readFileSync("../../voting/res/voting_contract.wasm")),
        nearAPI.transactions.functionCall("new", Buffer.from(JSON.stringify(newArgs)), 10000000000000, "0"),
    ]);
END
)

echo $REPL | near repl

