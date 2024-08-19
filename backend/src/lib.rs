mod bitcoin_api;
mod bitcoin_wallet;
mod ecdsa_api;
mod inscription;
mod schnorr_api;
mod types;

use ic_cdk::api::management_canister::bitcoin::BitcoinNetwork;

use std::cell::{Cell, RefCell};

thread_local! {
    // The bitcoin network to connect to.
    //
    // When developing locally this should be `Regtest`.
    // When deploying to the IC this should be `Testnet` or 'Mainnet'.
    static NETWORK: Cell<BitcoinNetwork> = Cell::new(BitcoinNetwork::Regtest);

    // The derivation path to use for ECDSA secp256k1.
    static DERIVATION_PATH: Vec<Vec<u8>> = vec![];

    // The ECDSA and Schnor key name.
    static KEY_NAME: RefCell<String> = RefCell::new(String::from(""));

}

#[ic_cdk::init]
pub fn init(network: BitcoinNetwork) {
    NETWORK.with(|n| n.set(network));

    KEY_NAME.with(|key_name| {
        key_name.replace(String::from(match network {
            // For local development, we use a special test key with dfx.
            BitcoinNetwork::Regtest => "dfx_test_key",
            // On the IC we're using the real threshold key.
            BitcoinNetwork::Mainnet | BitcoinNetwork::Testnet => "key_1",
        }))
    });
}

/// Returns the balance of the given bitcoin address.
#[ic_cdk::update]
pub async fn get_balance(address: String) -> u64 {
    let network = NETWORK.with(|n| n.get());
    bitcoin_api::get_balance(network, address).await
}

#[ic_cdk::update]
pub async fn inscribe(
    content_type: String,
    body: String,
    recipient: Option<String>,
    fee_rate: Option<u64>,
) -> (String, String) {
    let network = NETWORK.with(|n| n.get());
    let content_type = Some(content_type.as_bytes().to_vec());
    let body = Some(body.as_bytes().to_vec());
    bitcoin_wallet::inscribe(
        network,
        content_type,
        body,
        recipient,
        fee_rate.unwrap_or(10),
    )
    .await
}

#[ic_cdk::update]
pub async fn get_p2pkh_address() -> String {
    let derivation_path = DERIVATION_PATH.with(|d| d.clone());
    let key_name = KEY_NAME.with(|kn| kn.borrow().to_string());
    let network = NETWORK.with(|n| n.get());
    bitcoin_wallet::get_p2pkh_address(network, key_name, derivation_path).await
}

ic_cdk::export_candid!();
