use bip32::{Seed, XPrv};
use bitcoin::{
    key::{Secp256k1, UntweakedKeypair}, secp256k1::Message
};
use candid::{CandidType, Deserialize, Principal};

use serde::Serialize;

use ic_crypto_extended_bip32::{
    DerivationIndex, DerivationPath,
};

use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{DefaultMemoryImpl, StableCell};
use std::cell::RefCell;

use getrandom::{Error, register_custom_getrandom};


type Memory = VirtualMemory<DefaultMemoryImpl>;

#[derive(CandidType, Deserialize, Serialize, Debug)]
struct SchnorrPublicKey {
    pub canister_id: Option<Principal>,
    pub derivation_path: Vec<Vec<u8>>,
    pub key_id: SchnorrKeyId,
}

#[derive(CandidType, Deserialize, Debug)]
struct SchnorrPublicKeyReply {
    pub public_key: Vec<u8>,
    pub chain_code: Vec<u8>,
}

#[derive(CandidType, Deserialize, Serialize, Debug)]
struct SignWithSchnorr {
    pub message: Vec<u8>,
    pub derivation_path: Vec<Vec<u8>>,
    pub key_id: SchnorrKeyId,
}

// enum SchnorrKeyIds {
//     #[allow(unused)]
//     TestKey1,
// }

// impl SchnorrKeyIds {
//     fn to_key_id(&self) -> SchnorrKeyId {
//         SchnorrKeyId {
//             name: match self {
//                 Self::TestKey1 => "test_key_1",
//             }
//             .to_string(),
//         }
//     }
// }

#[derive(CandidType, Deserialize, Debug)]
struct SignWithSchnorrReply {
    pub signature: Vec<u8>,
}

#[derive(CandidType, Deserialize, Serialize, Debug, Clone)]
struct SchnorrKeyId {
    pub name: String,
}

thread_local! {
    // The memory manager is used for simulating multiple memories. Given a `MemoryId` it can
    // return a memory that can be used by stable structures.
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));

    static LOCK: RefCell<bool> = RefCell::new(false);

    // Initialize a `StableCell` with `MemoryId(0)`.
    static SEED: RefCell<StableCell<[u8; 64], Memory>> = RefCell::new(
            StableCell::init(
                MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))),
                [0; 64]
        ).unwrap()
    );
}

#[ic_cdk::update]
async fn init_key() -> () {
    SEED.with(|s| {
        let seed = s.borrow().get().clone();
        let is_initialized = seed != [0; 64];

        if is_initialized {
            ic_cdk::trap("Already initialized");
        }
    });

    LOCK.with_borrow_mut(|l| {
        if *l {
            ic_cdk::trap("Already initializing");
        }
        *l = true;
    });

    let mut rand = match ic_cdk::api::management_canister::main::raw_rand().await {
        Ok(rand) => {
            LOCK.with_borrow_mut(|l| {
                *l = false;
            });
            rand.0
        }
        Err(err) => {
            LOCK.with_borrow_mut(|l| {
                *l = false;
            });
            ic_cdk::trap(&format!("Error: {:?}", err));
        }
    };

    rand.extend(rand.clone());
    let rand: [u8; 64] = rand.try_into().expect("Expected a Vec of length 64");

    let seed = Seed::new(rand);

    SEED.with(|s| {
        s.borrow_mut().set(seed.as_bytes().to_owned()).unwrap();
    });
}

// #[ic_cdk::update]
// fn schnorr_public_key(_arg: SchnorrPublicKey) -> SchnorrPublicKeyReply {
//     let seed = SEED.with(|s| s.borrow().get().clone());
//     let seed = Seed::new(seed);

//     let root_xprv = XPrv::new(&seed).unwrap();
//     let key_bytes = root_xprv.private_key().to_bytes();

//     let signing_key = k256::schnorr::SigningKey::from_bytes(key_bytes.as_slice()).unwrap();
//     let verifying_key  = signing_key.verifying_key();

//     let chain_code = Vec::new();


//     SchnorrPublicKeyReply {
//         public_key: verifying_key.to_bytes().to_vec(),
//         chain_code,
//     }
// }

// #[ic_cdk::update]
// fn sign_with_schnorr(arg: SignWithSchnorr) -> SignWithSchnorrReply {

//     let message = arg.message;

//     let seed = SEED.with(|s| s.borrow().get().clone());
//     let seed = Seed::new(seed);

//     let root_xprv = XPrv::new(&seed).unwrap();
//     let key_bytes = root_xprv.private_key().to_bytes();

//     let signing_key = k256::schnorr::SigningKey::from_bytes(key_bytes.as_slice()).unwrap();
    
//     let signature = signing_key.sign(message.as_slice());

//     SignWithSchnorrReply {
//         signature: signature.to_bytes().to_vec(),
//     }
// }



#[ic_cdk::update]
fn schnorr_public_key(arg: SchnorrPublicKey) -> SchnorrPublicKeyReply {

    let secp256k1: Secp256k1<bitcoin::secp256k1::All> = Secp256k1::new();

    let seed = SEED.with(|s| s.borrow().get().clone());
    let seed = Seed::new(seed);
    
    let root_xprv = XPrv::new(&seed).unwrap();
    let key_bytes = root_xprv.private_key().to_bytes();
    
    let key_pair = UntweakedKeypair::from_seckey_slice(&secp256k1, &key_bytes).expect("Should generate key pair");

    // let (public_key, parity) = XOnlyPublicKey::from_keypair(&key_pair);
    
    let master_chain_code = [0u8; 32];

    let canister_id = match arg.canister_id {
        Some(canister_id) => {
            canister_id
        }
        None => {
           ic_cdk::caller()
        }
    };

    let public_key_sec1 = key_pair.public_key().serialize();
    let mut path = vec![];
    let derivation_index = DerivationIndex(canister_id.as_slice().to_vec());
    path.push(derivation_index);

    for index in arg.derivation_path {
        path.push(DerivationIndex(index));
    }
    let derivation_path = DerivationPath::new(path);

    let res = derivation_path
        .key_derivation(
            &public_key_sec1,
            &master_chain_code,
        )
        .expect("Should derive key");

    SchnorrPublicKeyReply {
        public_key: res.derived_public_key,
        chain_code: res.derived_chain_code,
    }

}


#[ic_cdk::update]
fn sign_with_schnorr(arg: SignWithSchnorr) -> SignWithSchnorrReply {

    let message = arg.message;

    let seed = SEED.with(|s| s.borrow().get().clone());
    let seed = Seed::new(seed);

    let root_xprv = XPrv::new(&seed).unwrap();
    let private_key_bytes = root_xprv.private_key().to_bytes();

    let master_chain_code = [0u8; 32];

    let canister_id = ic_cdk::caller();

    let mut path = vec![];
    let derivation_index = DerivationIndex(canister_id.as_slice().to_vec());
    path.push(derivation_index);

    for index in arg.derivation_path {
        path.push(DerivationIndex(index));
    }
    let derivation_path = DerivationPath::new(path);

    let res = derivation_path
        .private_key_derivation(
            &private_key_bytes,
            &master_chain_code,
        )
        .expect("Should derive key");
    
    let secp256k1: Secp256k1<bitcoin::secp256k1::All> = Secp256k1::new();
    let key_pair = UntweakedKeypair::from_seckey_slice(&secp256k1, &res.derived_private_key).expect("Should generate key pair");

    let sig = secp256k1.sign_schnorr_no_aux_rand(
        &Message::from_digest_slice(message.as_ref())
          .expect("should be cryptographically secure hash"),
        &key_pair,
      );

    SignWithSchnorrReply {
        signature: sig.serialize().to_vec(),
    }

}

pub fn my_custom_random(_buf: &mut [u8]) -> Result<(), Error> {
    ic_cdk::trap("Not implemented");
}

register_custom_getrandom!(my_custom_random);

ic_cdk::export_candid!();
