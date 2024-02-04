#!/bin/bash

# Build the schnorr canister
cargo build --release --target wasm32-unknown-unknown --package schnorr_canister

# Extract the candid file
candid-extractor target/wasm32-unknown-unknown/release/schnorr_canister.wasm > schnorr_canister/schnorr_canister.did

# Create and deploy the canister
#dfx canister create schnorr_canister
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