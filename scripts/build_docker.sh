#!/usr/bin/env bash

# Exit script as soon as a command fails.
set -ex

NAME="$1"
CONTRACT_WASM_NAME="$2"

if docker ps -a --format '{{.Names}}' | grep -Eq "^build_${NAME}\$"; then
    echo "Container exists"
else
docker create \
     --mount type=bind,source=$(pwd),target=/host \
     --cap-add=SYS_PTRACE --security-opt seccomp=unconfined \
     --name=build_$NAME \
     -w /host/$NAME \
     -e RUSTFLAGS='-C link-arg=-s' \
     -it nearprotocol/contract-builder \
     /bin/bash
fi

docker start build_$NAME
docker exec build_$NAME /bin/bash -c "rustup toolchain install stable-2020-10-08; rustup default stable-2020-10-08; rustup target add wasm32-unknown-unknown; cargo build --target wasm32-unknown-unknown --release"

mkdir -p res
cp $NAME/target/wasm32-unknown-unknown/release/$CONTRACT_WASM_NAME.wasm $NAME/res/$CONTRACT_WASM_NAME.wasm
