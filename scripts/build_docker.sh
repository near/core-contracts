#!/usr/bin/env bash

# Exit script as soon as a command fails.
set -e

DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"
NAME="$1"
CONTRACT_WASM_NAME="$2"

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
cp $DIR/../$NAME/target/wasm32-unknown-unknown/release/$CONTRACT_WASM_NAME.wasm $DIR/../$NAME/res/$CONTRACT_WASM_NAME.wasm
