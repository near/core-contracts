#!/bin/bash
set -e

# Build with all features
cargo build --target wasm32-unknown-unknown --release --all-features
cp target/wasm32-unknown-unknown/release/state_manipulation.wasm ./res/state_manipulation.wasm

# Build with just cleanup
cargo build --target wasm32-unknown-unknown --release --no-default-features --features cleanup
cp target/wasm32-unknown-unknown/release/state_manipulation.wasm ./res/state_cleanup.wasm

# Build with just state replace
cargo build --target wasm32-unknown-unknown --release --no-default-features --features replace
cp target/wasm32-unknown-unknown/release/state_manipulation.wasm ./res/state_replace.wasm