#!/bin/bash


# Build the backend canister
cargo build --release --target wasm32-unknown-unknown --package backend
candid-extractor target/wasm32-unknown-unknown/release/backend.wasm > backend/backend.did

# Deploy the backend canister
dfx deploy backend --argument "( variant { regtest } )" --mode=reinstall -y

dfx generate
dfx deploy frontend