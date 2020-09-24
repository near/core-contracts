#!/bin/bash
set -e

# nearup localnet --num-nodes 5 --docker-image "nearprotocol/nearcore:master" --overwrite
nearup localnet --num-nodes 5 --binary-path /Users/ekwork/code/nearcore/target/debug/ --overwrite

export MASTER_ACCOUNT_ID=node0
export NEAR_ENV=local

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

VOTE_ACCOUNT_ID="vote.$MASTER_ACCOUNT_ID"

check_votes() {
  echo "Checking votes"
  near view $VOTE_ACCOUNT_ID get_total_voted_stake
  near view $VOTE_ACCOUNT_ID get_votes
  echo "Checking result"
  near view $VOTE_ACCOUNT_ID get_result

}

vote() {
  ACCOUNT_ID="node${1}"
  echo "Voting through the pool to node $ACCOUNT_ID"
  near call $ACCOUNT_ID vote "{\"voting_account_id\": \"$VOTE_ACCOUNT_ID\", \"is_vote\": true}" --accountId=$OWNER_ACCOUNT_ID --gas=200000000000000

  check_votes
}

vote 1
vote 2

echo "Going to kick out node1. And restake with node0"
near call node1 pause_staking --accountId=$OWNER_ACCOUNT_ID
sleep 1
near stake node0 "$NODE0_PUBLIC_KEY" 999000000

echo "Sleeping 3+ minutes (for 3+ epochs)"
sleep 200

echo "Current validators should be the 3 nodes with the staking pools and node0"
near validators current
near validators current | grep "Validators (total: 4,"

check_votes

vote 3
vote 4

stop_nodes
