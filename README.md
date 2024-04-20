# Ordinals Inscription Canister

This example project explores the possibility of inscribing ordinal inscriptions onto the Bitcoin blockchain using the Internet Computer Protocol (ICP).

Inscriptions are made by spending a Pay-to-Taproot (P2TR) output, which necessitates the use of Schnorr signatures. Currently, ICP's Chain-Key Signature suite does not support Schnorr signatures. As a workaround, this project uses an experimental Schnorr Canister for signing transactions. This canister generates a private key from a seed, which is derived from ICP's source of unbiased randomness. It's important to note that this method is not secured by the canister's controller. Consequently, there's a risk that node providers could access the canister's state and extract the private key.

This project has only been tested on the local development environment on a Mac with Apple Silicon. It may not work on other platforms. Please file an issue if you encounter any problems.

## Quick Start

Make sure that [Node.js](https://nodejs.org/en/) `>= 16.x`, [`dfx`](https://internetcomputer.org/docs/current/developer-docs/build/install-upgrade-remove) `>= 0.12.x`, [Rust](https://www.rust-lang.org/tools/install), and [Docker](https://docs.docker.com/get-docker/) (including docker compose) are installed on your system.

After installing Rust, run these commands to configure your system for IC canister development:

```sh
rustup target add wasm32-unknown-unknown # Required for building IC canisters
cargo install cargo-watch # Optional; used for live reloading in `npm start`
```

Next, make sure Docker is running, and then run the following commands to start Bitcoin and Ord:

```sh
./init.sh
```

If you are on Apple silicon, you need to use platform emulation:

```sh
 DOCKER_DEFAULT_PLATFORM=linux/amd64 ./init.sh
 ```

Start the local `dfx` replica, with:

```sh
dfx start --background
```

Then, start a proxy to be able to connect from the frontend to the local Bitcoin RPC server:

```sh
npm install
npm run proxy
```

and build and deploy the canisters:

```sh
./deploy.sh
```

Finally, you should see the following:

```
Deployed canisters.
URLs:
  Frontend canister via browser
    frontend: http://be2us-64aaa-aaaaa-qaabq-cai.localhost:4943/
  Backend canister via Candid interface:
    backend: http://bnz7o-iuaaa-aaaaa-qaaaa-cai.localhost:4943/?id=bd3sg-teaaa-aaaaa-qaaba-cai
    schnorr_canister: http://bnz7o-iuaaa-aaaaa-qaaaa-cai.localhost:4943/?id=6fwhw-fyaaa-aaaap-qb7ua-cai
```

You can open the frontend in your browser by visiting the URL provided.

Optionally, you can start a local development frontend with hot reload accessible at [http://localhost:3000](http://localhost:3000) by running:

```sh
npm run frontend
```

### Stopping the service

```sh
docker compose down
dfx stop
```

Stop the proxy and frontend processes manually.
If you want to remove the data from the `bitcoind` and `ord` containers, you can run `docker compose down -v`.

## Architecture 

![Architecture](/docs/inscriptions_architecture.svg)

### Frontend

![Frontend](/docs/frontend.png)


## How it works

Ordinal inscriptions are created by spending a Pay-to-Taproot (P2TR) output. Therefore, we have to create two Bitcoin transactions. The first transaction, the commit transaction, spends one or more (P2PKH) outputs controlled by the inscription canister via ECDSA signatures and creates a P2TR output that commits to a reveal script containing the ordinal inscription. The second transaction, the reveal transaction, spends the P2TR output and reveals the ordinal inscription by providing the reveal script and a Schnorr signature. The transaction creates a new output associated with the destination address, which effectively owns the ordinal inscription.

![Transactions](/docs/transactions.svg)


## Credits

The code in this repository is based on the following projects:

- [DFINITY Basic Bitcoin sample project](https://github.com/dfinity/examples/tree/master/rust/basic_bitcoin).
- [Ord](https://github.com/ordinals/ord).