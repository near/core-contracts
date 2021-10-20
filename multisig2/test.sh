#!/bin/bash

./build.sh
cargo +stable test -- --nocapture
