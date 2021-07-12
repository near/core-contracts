#!/usr/bin/env bash

# Exit script as soon as a command fails.
set -e

DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"

NAME="staking-pool"

if docker ps -a --format '{{.Names}}' | grep -Eq "^build_${NAME}\$"; then
    echo "Container exists"
else
docker create \
     --mount type=bind,source=$DIR/..,target=/host \
     --cap-add=SYS_PTRACE --security-opt seccomp=unconfined \
     --name=build_$NAME \
     -w /host/$NAME \
     -e RUSTFLAGS='-C link-arg=-s' \
     -it nearprotocol/contract-builder \
     /bin/bash
fi

docker start build_$NAME
docker exec -it build_$NAME /bin/bash -c "cargo build --target wasm32-unknown-unknown --release"

mkdir -p res
cp $DIR/../target/wasm32-unknown-unknown/release/$NAME.wasm $DIR/../res/$NAME.wasm
