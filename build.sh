#!/bin/bash

RUSTFLAGS='-C link-arg=-s' cargo +stable build -p near-evm --target wasm32-unknown-unknown --release || exit 1
mkdir -p res
cp target/wasm32-unknown-unknown/release/near_evm.wasm ./res/

# wasm-opt -Oz --output ./res/near_evm.wasm ./res/near_evm.wasm
# rm -rf target
