#!/bin/bash
set -e
source ../flags.sh
cargo +stable build --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/voting_contract.wasm ./res/
