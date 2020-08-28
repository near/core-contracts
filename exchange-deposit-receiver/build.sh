#!/bin/bash
set -e

RUSTFLAGS='-C link-arg=-s' cargo +nightly build --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/exchange_deposit_receiver.wasm ./res/
