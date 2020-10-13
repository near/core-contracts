#!/bin/bash
set -e
cd "`dirname $0`"

RUSTFLAGS='-C link-arg=-s' cargo +stable build --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/uniswap.wasm ./res/

