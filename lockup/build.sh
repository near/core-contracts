#!/bin/bash
set -e

RUSTFLAGS='-C link-arg=-s' cargo +stable-2020-10-08 build --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/lockup_contract.wasm res/

