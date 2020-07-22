#!/bin/bash
set -e

nearup localnet --num-nodes 5 --docker-image "nearprotocol/nearcore:master" --overwrite
export MASTER_ACCOUNT_ID=node0
export NODE_ENV=local

stop_nodes() {
  echo "STOOOP THE NODES!"
  nearup stop
}

trap "stop_nodes" ERR

LAST_NODE=4
NODES_TO_VOTE=3

echo "Awaiting for network to start"
sleep 3

echo "Current validator should be the $LAST_NODE + 1 nodes"
near validators current

for (( i=0; i<=$LAST_NODE; i++ )); do
  cp ~/.near/localnet/node$i/node_key.json ~/.near-credentials/local/node$i.json
done;

OWNER_ACCOUNT_ID="owner.$MASTER_ACCOUNT_ID"
near create-account $OWNER_ACCOUNT_ID --masterAccount=$MASTER_ACCOUNT_ID --initialBalance=10000

echo "Deploying core accounts/"
(cd .. && ./deploy_core.sh)

for (( i=1; i<=$LAST_NODE; i++ )); do
  ACCOUNT_ID="node${i}"
  near stake $ACCOUNT_ID "ed25519:7PGseFbWxvYVgZ89K1uTJKYoKetWs7BJtbyXDzfbAcqX" 0
done;

NODE0_PUBLIC_KEY=$(grep -oE 'ed25519:[^"]+' ~/.near/localnet/node0/validator_key.json | head -1)
echo "Staking close to 1B NEAR by node0, to avoid it being kicked out too fast."
near stake node0 "$NODE0_PUBLIC_KEY" 999000000

echo "Sleeping 3+ minutes (for 3+ epochs)"
sleep 200

echo "The only current validator should be the node0"
near validators current

for (( i=1; i<=$LAST_NODE; i++ )); do
  ACCOUNT_ID="node${i}"
  near deploy --wasmFile="../../staking-pool/res/staking_pool.wasm" --accountId=$ACCOUNT_ID
  PUBLIC_KEY=$(grep -oE 'ed25519:[^"]+' ~/.near/localnet/node$i/validator_key.json | head -1)
  near call $ACCOUNT_ID new "{\"owner_id\": \"$OWNER_ACCOUNT_ID\", \"stake_public_key\": \"$PUBLIC_KEY\", \"reward_fee_fraction\": {\"numerator\": 10, \"denominator\": 100}}" --accountId=$OWNER_ACCOUNT_ID
  sleep 1
done;

echo "Deployed pools and staked a lot. Sleep for 1 minute."
sleep 70

echo "Going to ping pools in case the stake was lost due to seat assignment"

for (( i=1; i<=$LAST_NODE; i++ )); do
  ACCOUNT_ID="node${i}"
  near call $ACCOUNT_ID ping "{}" --accountId=$OWNER_ACCOUNT_ID
  sleep 1
done;

echo "Unstaking for node0"
near stake node0 "$NODE0_PUBLIC_KEY" 0

echo "Sleeping 3+ minutes (for 3+ epochs)"
sleep 200

echo "Current validators should be the $LAST_NODE nodes with the staking pools only"
near validators current
near validators current | grep "Validators (total: $LAST_NODE,"

echo "Checking votes (should be none)"
VOTE_ACCOUNT_ID="vote.$MASTER_ACCOUNT_ID"
near view $VOTE_ACCOUNT_ID get_total_voted_stake
near view $VOTE_ACCOUNT_ID get_votes

for (( i=1; i<=$NODES_TO_VOTE; i++ )); do
  ACCOUNT_ID="node${i}"
  echo "Voting through the pool to node $ACCOUNT_ID"
  near call $ACCOUNT_ID vote "{\"voting_account_id\": \"$VOTE_ACCOUNT_ID\", \"is_vote\": true}" --accountId=$OWNER_ACCOUNT_ID --gas=200000000000000

  echo "Checking votes again"
  near view $VOTE_ACCOUNT_ID get_total_voted_stake
  near view $VOTE_ACCOUNT_ID get_votes
  echo "Checking result"
  near view $VOTE_ACCOUNT_ID get_result
done;

stop_nodes
