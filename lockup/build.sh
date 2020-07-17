#!/bin/bash
set -e
cd "`dirname $0`"
source ../flags.sh
cargo +stable build --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/lockup_contract.wasm res/

