#!/bin/bash

# Start bitcoind 
docker compose up bitcoind -d
sleep 2

# Create wallet and load it. This might throw an error if the wallet already exists, but that's fine.
docker compose exec bitcoind bitcoin-cli -regtest createwallet "testwallet"
docker compose exec bitcoind bitcoin-cli -regtest loadwallet "testwallet"

# Generate 101 blocks to make sure we have some coins to spend
height=$(docker compose exec bitcoind bitcoin-cli -regtest getblockcount)
if [ $height -lt 101 ]; then
    docker compose exec bitcoind bitcoin-cli -regtest -generate 101
fi

# Start the ord service. It'll be available at localhost:8080
docker compose up ord -d