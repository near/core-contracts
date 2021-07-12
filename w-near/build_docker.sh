#!/usr/bin/env bash

# Exit script as soon as a command fails.
set -e

DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"

NAME="w-near"

if docker ps -a --format '{{.Names}}' | grep -Eq "^${NAME}\$"; then
    echo "Container exists"
else
docker create \
     --mount type=bind,source=$DIR/..,target=/host \
     --cap-add=SYS_PTRACE --security-opt seccomp=unconfined \
     --name=$NAME \
     -w /host/lockup \
     -e RUSTFLAGS='-C link-arg=-s' \
     -it nearprotocol/contract-builder \
     /bin/bash
fi

docker start $NAME
docker exec -it $NAME /bin/bash -c "cargo build --target wasm32-unknown-unknown --release"

mkdir -p res
cp $DIR/../target/wasm32-unknown-unknown/release/lockup_contract.wasm $DIR/../res/lockup_contract.wasm

RUSTFLAGS='-C link-arg=-s' cargo build --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/lockup_contract.wasm res/
