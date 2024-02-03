use crate::{bitcoin_api, ecdsa_api, inscription::Inscription, schnorr_api, KEY_NAME};
use bitcoin::{
    absolute::LockTime,
    blockdata::{opcodes, script::Builder, witness::Witness},
    consensus::serialize,
    hashes::Hash,
    key::{Secp256k1, UntweakedPublicKey},
    script::PushBytesBuf,
    secp256k1::{schnorr, XOnlyPublicKey},
    sighash::{self, SighashCache, TapSighashType},
    taproot::{ControlBlock, LeafVersion, Signature, TaprootBuilder},
    transaction::Version,
    Address, AddressType, Amount, EcdsaSighashType, FeeRate, Network, OutPoint, Script, Sequence, TapLeafHash,
    Transaction, TxIn, TxOut, Txid,
};

use hex::ToHex;
use ic_cdk::api::management_canister::bitcoin::{BitcoinNetwork, Utxo};
use ic_cdk::print;
use sha2::Digest;
use std::str::FromStr;

const SIG_HASH_TYPE: EcdsaSighashType = EcdsaSighashType::All;

/// The size of a schnorr signature.
pub const SCHNORR_SIGNATURE_SIZE: usize = 64;

/// Returns the P2PKH address of this canister at the given derivation path.
/// We use this to generate payment addresses
pub async fn get_p2pkh_address(
    network: BitcoinNetwork,
    key_name: String,
    derivation_path: Vec<Vec<u8>>,
) -> String {
    // Fetch the public key of the given derivation path.
    let public_key = ecdsa_api::ecdsa_public_key(key_name, derivation_path).await;

    // Compute the address.
    public_key_to_p2pkh_address(network, &public_key)
}

fn transform_network(network: BitcoinNetwork) -> Network {
    match network {
        BitcoinNetwork::Mainnet => Network::Bitcoin,
        BitcoinNetwork::Testnet => Network::Testnet,
        BitcoinNetwork::Regtest => Network::Regtest,
    }
}

pub async fn inscribe(
    network: BitcoinNetwork,
    content_type: Option<Vec<u8>>,
    body: Option<Vec<u8>>,
    dst_address: Option<String>,
    fee_rate: u64,
) -> (String, String) {
    let bitcoin_network = transform_network(network);
    let inscription = Inscription::new(content_type, body);

    let key_name = KEY_NAME.with(|kn| {
        kn.borrow().to_string()
    });

    let derivation_path = vec![];

    // Fetch our public key, P2PKH address, and UTXOs.
    let own_public_key =
        ecdsa_api::ecdsa_public_key(key_name.clone(), derivation_path.clone()).await;
    let own_address = public_key_to_p2pkh_address(network, &own_public_key);

    print("Fetching UTXOs...");
    // Note that pagination may have to be used to get all UTXOs for the given address.
    // For the sake of simplicity, it is assumed here that the `utxo` field in the response
    // contains all UTXOs.
    let own_utxos = bitcoin_api::get_utxos(network, own_address.clone())
        .await
        .utxos;

    // We can be sure that the address corresponds to the correct network
    let own_address = Address::from_str(&own_address).unwrap().assume_checked();

    let dst_address = if let Some(dst_address) = dst_address {
       Address::from_str(&dst_address).unwrap().assume_checked()
    } else {
        // Send inscription to canister's own address if none is provided
        own_address.clone()  
    };

    print("Fetching Schnorr public key...");
    let raw_public_key = schnorr_api::schnorr_public_key(key_name.clone(), vec![]).await;
    let schnorr_public_key = UntweakedPublicKey::from_slice(&raw_public_key).unwrap();


    let fee_rate = FeeRate::from_sat_per_vb(fee_rate).unwrap();

    let (commit_tx, reveal_tx) = build_inscription_transactions(
        bitcoin_network,
        &own_utxos,
        &dst_address,
        schnorr_public_key,
        inscription,
        &own_public_key,
        &own_address,
        key_name,
        derivation_path,
        fee_rate
    )
    .await
    .expect("Should build inscription transactions");

   
    let commit_tx_bytes = serialize(&commit_tx);
    print(&format!(
        "Signed commit transaction: {}",
        hex::encode(&commit_tx_bytes)
    ));

    print("Sending commit transaction...");
    bitcoin_api::send_transaction(network, commit_tx_bytes).await;
    print("Done");

    let reveal_tx_bytes = serialize(&reveal_tx);
    print(&format!(
        "Signed reveal transaction: {}",
        hex::encode(&reveal_tx_bytes)
    ));

    print("Sending reveal transaction...");
    bitcoin_api::send_transaction(network, reveal_tx_bytes).await;
    print("Done");

    (commit_tx.txid().encode_hex(), reveal_tx.txid().encode_hex())

}


async fn build_inscription_transactions(
    network: Network,
    own_utxos: &[Utxo],
    dst_address: &Address,
    schnorr_public_key: XOnlyPublicKey,
    inscription: Inscription,
    own_public_key: &[u8],
    own_address: &Address,
    key_name: String,
    derivation_path: Vec<Vec<u8>>,
    fee_rate: FeeRate,
) -> Result<(Transaction, Transaction), String> {
    
    
    let mut builder = Builder::new();

    builder = inscription.append_reveal_script_to_builder(builder);

    ic_cdk::print(&format!("Reveal script: {}", &builder.clone().into_script()));

    let secp256k1 = Secp256k1::new();

    builder = builder
        .push_slice(&schnorr_public_key.serialize())
        .push_opcode(opcodes::all::OP_CHECKSIG);

    let reveal_script = builder.into_script();

    ic_cdk::print(&format!("Reveal script: {}", &reveal_script));

    let taproot_spend_info = TaprootBuilder::new()
        .add_leaf(0, reveal_script.clone())
        .expect("adding leaf should work")
        .finalize(&secp256k1, schnorr_public_key)
        .expect("finalizing taproot builder should work");

    let control_block = taproot_spend_info
        .control_block(&(reveal_script.clone(), LeafVersion::TapScript))
        .expect("should compute control block");


    let commit_tx_address = Address::p2tr_tweaked(taproot_spend_info.output_key(), network);

    let mut reveal_inputs = vec![OutPoint::null()];

    // Amount should be value of the UTXO minus the fees for commit and reveal transactions
    let mut reveal_outputs = vec![TxOut {
        script_pubkey: dst_address.script_pubkey(),
        value: Amount::from_sat(0),
    }];

    let commit_input_index = 0;

    let (_, reveal_fee) = build_reveal_transaction(
        &control_block,
        fee_rate,
        reveal_inputs.clone(),
        commit_input_index,
        reveal_outputs.clone(),
        &reveal_script,
    );

    // Select which UTXOs to spend. We naively spend the oldest available UTXOs,
    // even if they were previously spent in a transaction. This isn't a
    // problem as long as at most one transaction is created per block and
    // we're using min_confirmations of 1.
    let mut utxos_to_spend = vec![];
    let mut total_spent = 0;
    for utxo in own_utxos.iter().rev() {
        total_spent += utxo.value;
        utxos_to_spend.push(utxo);
    }

    let total_spent = Amount::from_sat(total_spent);

    let inputs: Vec<TxIn> = utxos_to_spend
        .into_iter()
        .map(|utxo| TxIn {
            previous_output: OutPoint {
                txid: Txid::from_raw_hash(Hash::from_slice(&utxo.outpoint.txid).unwrap()),
                vout: utxo.outpoint.vout,
            },
            sequence: Sequence::ZERO,
            witness: Witness::new(),
            script_sig: Script::new().into(),
        })
        .collect();

    let mut unsigned_commit_tx = Transaction {
        input: inputs,
        output: vec![TxOut {
            script_pubkey: commit_tx_address.script_pubkey(),
            value: total_spent,
        }],
        lock_time: LockTime::ZERO,
        version: Version(2),
    };

    // We assume that we spend a single P2PKH output
    let sig_vbytes = 73;

    let commit_fee = fee_rate
      .fee_vb(unsigned_commit_tx.vsize() as u64 + sig_vbytes).unwrap();

    unsigned_commit_tx.output[0].value = total_spent - commit_fee;


    let commit_tx = sign_transaction_p2pkh(
        &own_public_key,
        &own_address,
        unsigned_commit_tx,
        key_name,
        derivation_path,
        ecdsa_api::sign_with_ecdsa,
    )
    .await;

    let (vout, _commit_output) = commit_tx
        .output
        .iter()
        .enumerate()
        .find(|(_vout, output)| output.script_pubkey == commit_tx_address.script_pubkey())
        .expect("should find sat commit/inscription output");

    reveal_inputs[commit_input_index] = OutPoint {
        txid: commit_tx.txid(),
        vout: vout.try_into().unwrap(),
    };

    reveal_outputs = vec![TxOut {
        script_pubkey: dst_address.script_pubkey(),
        value: total_spent - commit_fee - reveal_fee,
    }];

    let mut reveal_tx = Transaction {
        input: reveal_inputs
            .iter()
            .map(|outpoint| TxIn {
                previous_output: *outpoint,
                script_sig: Builder::new().into_script(),
                witness: Witness::new(),
                sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            })
            .collect(),
        output: reveal_outputs,
        lock_time: LockTime::ZERO,
        version: Version(2),
    };

    let mut sighasher = SighashCache::new(&mut reveal_tx);
    let sighash = sighasher
        .taproot_script_spend_signature_hash(
            commit_input_index,
            &sighash::Prevouts::All(commit_tx.output.as_slice()),
            TapLeafHash::from_script(&reveal_script, LeafVersion::TapScript),
            TapSighashType::Default,
        )
        .expect("failed to construct sighash");

    let msg = sighash.to_byte_array().to_vec();

    let sig = schnorr_api::sign_with_schnorr(String::from("test_key_1"), vec![], msg).await;

    let witness = sighasher
      .witness_mut(commit_input_index)
      .expect("getting mutable witness reference should work");

    witness.push(
      Signature {
        sig: schnorr::Signature::from_slice(sig.as_slice()).expect("should parse signature"),
        hash_ty: TapSighashType::Default,
      }
      .to_vec(),
    );

    witness.push(reveal_script);
    witness.push(&control_block.serialize());

    Ok((commit_tx, reveal_tx))
}

fn build_reveal_transaction(
    control_block: &ControlBlock,
    fee_rate: FeeRate,
    inputs: Vec<OutPoint>,
    commit_input_index: usize,
    outputs: Vec<TxOut>,
    script: &Script,
) -> (Transaction, Amount) {
    let reveal_tx = Transaction {
        input: inputs
            .iter()
            .map(|outpoint| TxIn {
                previous_output: *outpoint,
                script_sig: Builder::new().into_script(),
                witness: Witness::new(),
                sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            })
            .collect(),
        output: outputs,
        lock_time: LockTime::ZERO,
        version: Version(2),
    };

    let fee = {
        let mut reveal_tx = reveal_tx.clone();

        for (current_index, txin) in reveal_tx.input.iter_mut().enumerate() {
            // add dummy inscription witness for reveal input/commit output
            if current_index == commit_input_index {
                txin.witness.push(
                    Signature::from_slice(&[0; SCHNORR_SIGNATURE_SIZE])
                        .unwrap()
                        .to_vec(),
                );
                txin.witness.push(script);
                txin.witness.push(&control_block.serialize());
            } else {
                txin.witness = Witness::from_slice(&[&[0; SCHNORR_SIGNATURE_SIZE]]);
            }
        }

        fee_rate
            .fee_vb(reveal_tx.vsize().try_into().unwrap())
            .unwrap()
    };

    (reveal_tx, fee)
}


// Sign a P2PKH bitcoin transaction.
//
// IMPORTANT: This method is for demonstration purposes only and it only
// supports signing transactions if:
//
// 1. All the inputs are referencing outpoints that are owned by `own_address`.
// 2. `own_address` is a P2PKH address.
async fn sign_transaction_p2pkh<SignFun, Fut>(
    own_public_key: &[u8],
    own_address: &Address,
    mut transaction: Transaction,
    key_name: String,
    derivation_path: Vec<Vec<u8>>,
    signer: SignFun,
) -> Transaction
where
    SignFun: Fn(String, Vec<Vec<u8>>, Vec<u8>) -> Fut,
    Fut: std::future::Future<Output = Vec<u8>>,
{
    // Verify that our own address is P2PKH.
    assert_eq!(
        own_address.address_type(),
        Some(AddressType::P2pkh),
        "This example supports signing p2pkh addresses only."
    );

    let txclone = transaction.clone();
    for (index, input) in transaction.input.iter_mut().enumerate() {
        let sighash = SighashCache::new(&txclone)
            .legacy_signature_hash(index, &own_address.script_pubkey(), SIG_HASH_TYPE.to_u32())
            .unwrap();

        let signature = signer(
            key_name.clone(),
            derivation_path.clone(),
            sighash.as_byte_array().to_vec(),
        )
        .await;

        // Convert signature to DER.
        let der_signature = sec1_to_der(signature);

        let mut sig_with_hashtype = der_signature;
        sig_with_hashtype.push(SIG_HASH_TYPE.to_u32() as u8);

        let sig_with_hashtype_push_bytes = PushBytesBuf::try_from(sig_with_hashtype).unwrap();
        let own_public_key_push_bytes = PushBytesBuf::try_from(own_public_key.to_vec()).unwrap();
        input.script_sig = Builder::new()
            .push_slice(sig_with_hashtype_push_bytes)
            .push_slice(own_public_key_push_bytes)
            .into_script();
        input.witness.clear();
    }

    transaction
}

fn sha256(data: &[u8]) -> Vec<u8> {
    let mut hasher = sha2::Sha256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}
fn ripemd160(data: &[u8]) -> Vec<u8> {
    let mut hasher = ripemd::Ripemd160::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}

// Converts a public key to a P2PKH address.
fn public_key_to_p2pkh_address(network: BitcoinNetwork, public_key: &[u8]) -> String {
    // SHA-256 & RIPEMD-160
    let result = ripemd160(&sha256(public_key));

    let prefix = match network {
        BitcoinNetwork::Testnet | BitcoinNetwork::Regtest => 0x6f,
        BitcoinNetwork::Mainnet => 0x00,
    };
    let mut data_with_prefix = vec![prefix];
    data_with_prefix.extend(result);

    let checksum = &sha256(&sha256(&data_with_prefix.clone()))[..4];

    let mut full_address = data_with_prefix;
    full_address.extend(checksum);

    bs58::encode(full_address).into_string()
}


// Converts a SEC1 ECDSA signature to the DER format.
fn sec1_to_der(sec1_signature: Vec<u8>) -> Vec<u8> {
    let r: Vec<u8> = if sec1_signature[0] & 0x80 != 0 {
        // r is negative. Prepend a zero byte.
        let mut tmp = vec![0x00];
        tmp.extend(sec1_signature[..32].to_vec());
        tmp
    } else {
        // r is positive.
        sec1_signature[..32].to_vec()
    };

    let s: Vec<u8> = if sec1_signature[32] & 0x80 != 0 {
        // s is negative. Prepend a zero byte.
        let mut tmp = vec![0x00];
        tmp.extend(sec1_signature[32..].to_vec());
        tmp
    } else {
        // s is positive.
        sec1_signature[32..].to_vec()
    };

    // Convert signature to DER.
    vec![
        vec![0x30, 4 + r.len() as u8 + s.len() as u8, 0x02, r.len() as u8],
        r,
        vec![0x02, s.len() as u8],
        s,
    ]
    .into_iter()
    .flatten()
    .collect()
}