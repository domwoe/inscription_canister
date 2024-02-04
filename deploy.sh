#!/bin/bash


dfx deploy schnorr_canister

# Get the canister id
schnorr_canister_id=$(dfx canister id schnorr_canister)

# Build the backend canister
cargo build --release --target wasm32-unknown-unknown --package backend
candid-extractor target/wasm32-unknown-unknown/release/backend.wasm > backend/backend.did

# Deploy the backend canister and set the schnorr canister id
dfx deploy backend --argument "( variant { regtest }, \"${schnorr_canister_id}\" )" --mode=reinstall -y

dfx generate
dfx deploy frontend