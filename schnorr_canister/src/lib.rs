use bip32::{Seed, XPrv};
use bitcoin::{
    key::{Secp256k1, UntweakedKeypair},
    secp256k1::Message,
};
use candid::{CandidType, Decode, Deserialize, Encode, Principal};

use serde::Serialize;
use serde_bytes::ByteBuf;

use ic_crypto_extended_bip32::{DerivationIndex, DerivationPath};

use ic_stable_structures::storable::Bound;
use ic_stable_structures::{StableBTreeMap, StableCell, Storable};

use std::borrow::Cow;
use std::cell::RefCell;
use std::time::Duration;

use getrandom::{register_custom_getrandom, Error};

mod memory;
use memory::Memory;

const MAX_VALUE_SIZE: u32 = 100;

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

enum SchnorrKeyIds {
    DfxTestKey,
    TestKey1,
}

impl SchnorrKeyIds {
    fn to_key_id(&self) -> SchnorrKeyId {
        SchnorrKeyId {
            name: match self {
                Self::DfxTestKey => "dfx_test_key",
                Self::TestKey1 => "test_key_1",
            }
            .to_string(),
        }
    }

    fn variants() -> Vec<SchnorrKeyIds> {
        vec![SchnorrKeyIds::DfxTestKey, SchnorrKeyIds::TestKey1]
    }
}

#[derive(CandidType, Deserialize, Debug)]
struct SignWithSchnorrReply {
    pub signature: Vec<u8>,
}

#[derive(CandidType, Deserialize, Serialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SchnorrKeyId {
    name: String,
}
impl Storable for SchnorrKeyId {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }

    const BOUND: Bound = Bound::Bounded {
        max_size: MAX_VALUE_SIZE,
        is_fixed_size: false,
    };
}

#[derive(Clone, Debug, CandidType, Deserialize)]
struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: ByteBuf,
    pub certificate_version: Option<u16>,
}

type HeaderField = (String, String);

#[derive(Clone, Debug, CandidType, Deserialize)]
struct HttpResponse {
    pub status_code: u16,
    pub headers: Vec<HeaderField>,
    pub body: ByteBuf,
}

#[derive(Serialize, Deserialize)]
struct State {
    // The seeds for the keys are stored in a stable memory.
    #[serde(skip, default = "init_stable_data")]
    seeds: StableBTreeMap<SchnorrKeyId, [u8; 64], Memory>,

    #[serde(skip, default = "init_sig_count")]
    sig_count: StableCell<u128, Memory>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

#[ic_cdk::init]
fn init() {
    ic_cdk_timers::set_timer(Duration::ZERO, move || {
        for key in SchnorrKeyIds::variants() {
            ic_cdk::spawn(async move {
                let seed = get_random_seed().await;
                STATE.with(|s| {
                    let seeds = &mut s.borrow_mut().seeds;
                    seeds
                        .get(&key.to_key_id())
                        .or_else(|| seeds.insert(key.to_key_id(), seed));
                });
            });
        }
    });
}
#[ic_cdk::update]
fn schnorr_public_key(arg: SchnorrPublicKey) -> SchnorrPublicKeyReply {
    let secp256k1: Secp256k1<bitcoin::secp256k1::All> = Secp256k1::new();

    let seed = Seed::new(STATE.with(|s| {
        s.borrow()
            .seeds
            .get(&arg.key_id)
            .expect(format!("No key with name {:?}", &arg.key_id).as_str())
            .clone()
    }));

    let root_xprv = XPrv::new(&seed).unwrap();
    let key_bytes = root_xprv.private_key().to_bytes();

    let key_pair = UntweakedKeypair::from_seckey_slice(&secp256k1, &key_bytes)
        .expect("Should generate key pair");

    let master_chain_code = [0u8; 32];

    let canister_id = match arg.canister_id {
        Some(canister_id) => canister_id,
        None => ic_cdk::caller(),
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
        .key_derivation(&public_key_sec1, &master_chain_code)
        .expect("Should derive key");

    SchnorrPublicKeyReply {
        public_key: res.derived_public_key,
        chain_code: res.derived_chain_code,
    }
}

#[ic_cdk::update]
fn sign_with_schnorr(arg: SignWithSchnorr) -> SignWithSchnorrReply {
    

    let message = arg.message;

    let seed = Seed::new(STATE.with(|s| {
        s.borrow()
            .seeds
            .get(&arg.key_id)
            .expect(format!("No key with name {:?}", &arg.key_id).as_str())
            .clone()
    }));

    // Increment the signature count
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        let current_count = state.sig_count.get().clone();
        let _ = state.sig_count.set(current_count + 1);
    });
    
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
        .private_key_derivation(&private_key_bytes, &master_chain_code)
        .expect("Should derive key");

    let secp256k1: Secp256k1<bitcoin::secp256k1::All> = Secp256k1::new();
    let key_pair = UntweakedKeypair::from_seckey_slice(&secp256k1, &res.derived_private_key)
        .expect("Should generate key pair");

    let sig = secp256k1.sign_schnorr_no_aux_rand(
        &Message::from_digest_slice(message.as_ref())
            .expect("should be cryptographically secure hash"),
        &key_pair,
    );

    SignWithSchnorrReply {
        signature: sig.serialize().to_vec(),
    }
}

#[ic_cdk::query]
fn http_request(_req: HttpRequest) -> HttpResponse {

    let sig_count = STATE.with(|s| s.borrow().sig_count.get().clone());
    let sig_count = format!("Signature count: {}", sig_count);

    HttpResponse {
        status_code: 200,
        headers: vec![("content-type".to_string(), "text/plain charset=utf-8".to_string())],
        body: ByteBuf::from(sig_count),
    }
}

fn init_sig_count() -> StableCell<u128, Memory> {
    StableCell::init(crate::memory::get_sig_count(), 0u128)
        .expect("Could not initialize sig count memory")
}

fn init_stable_data() -> StableBTreeMap<SchnorrKeyId, [u8; 64], Memory> {
    StableBTreeMap::init(crate::memory::get_seeds())
}

impl Default for State {
    fn default() -> Self {
        Self {
            sig_count: init_sig_count(),
            seeds: init_stable_data(),
        }
    }
}

async fn get_random_seed() -> [u8; 64] {
    match ic_cdk::api::management_canister::main::raw_rand().await {
        Ok(rand) => {
            let mut rand = rand.0;
            rand.extend(rand.clone());
            let rand: [u8; 64] = rand.try_into().expect("Expected a Vec of length 64");
            rand
        }
        Err(err) => {
            ic_cdk::trap(format!("Error getting random seed: {:?}", err).as_str());
        }
    }
}

pub fn my_custom_random(_buf: &mut [u8]) -> Result<(), Error> {
    ic_cdk::trap("Not implemented");
}

register_custom_getrandom!(my_custom_random);

ic_cdk::export_candid!();
