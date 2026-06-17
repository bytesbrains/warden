//! Generate a deterministic cross-language Veil fixture (gate-layer KAT, #184).
//!
//! Emits one JSON object on stdout that the `warden-wasm` Node test validates against — proving
//! the WASM `condition_identity` / `combine` / `open_gated` / `seal_gated` interoperate with
//! Rust-produced material byte-for-byte. Run: `cargo run -p warden-core --example veil_fixture`.
//!
//! Deterministic (seeded RNG) so the committed fixture is stable; regenerate if the format changes.

use ark_serialize::CanonicalSerialize;
use ark_std::rand::{rngs::StdRng, SeedableRng};
use serde_json::json;

use warden_core::condition::{Condition, Meta, Test};
use warden_core::dealer::deal;
use warden_core::envelope::seal_gated;
use warden_core::fed::FederationPublic;
use warden_core::ibe::{combine_verified, partial};

fn hex_canonical<T: CanonicalSerialize>(v: &T) -> String {
    let mut b = Vec::new();
    v.serialize_compressed(&mut b).unwrap();
    hex::encode(b)
}

fn main() {
    let mut rng = StdRng::seed_from_u64(0x5EED_BEEF); // fixed seed → stable fixture
    let network = "warden-fixture";
    let (t, n) = (2usize, 3usize);
    let fed = deal(t, n, &mut rng).unwrap();

    // The real Veil condition (matches core/tests/vectors.rs → id 47fce3a1…06cdb68e).
    let condition = Condition::Contract {
        chain: 84532,
        address: "0xb603C96D089F64Ac487EE0bdaE97D49848F86133".into(),
        func: "getHeartbeat(uint256)".into(),
        args: vec!["777".into()],
        word: 7,
        test: Test {
            cmp: "==".into(),
            value: json!(true),
        },
        meta: Meta {
            finality: 32,
            tier: 1,
        },
    };
    let id = condition.identity().unwrap();

    // An opaque "already-encrypted" blob (stand-in for Maktub's v2 hybrid envelope).
    let blob = b"\x02\x00\x01-fixture-hybrid-ciphertext-bytes-\xff\x00".to_vec();
    let env = seal_gated(condition.clone(), &fed.mpk, network, &blob, &mut rng).unwrap();

    // Partials from t-of-n nodes, and the combined key (what the client computes).
    let partials: Vec<String> = [0usize, 2]
        .iter()
        .map(|&i| hex_canonical(&partial(&fed.shares[i], &id)))
        .collect();
    let d_id = combine_verified(
        &[partial(&fed.shares[0], &id), partial(&fed.shares[2], &id)],
        &id,
        &fed.share_pubkeys,
    )
    .unwrap();

    let fed_pub = FederationPublic::new(network, t, n, &fed.mpk, &fed.share_pubkeys);
    let out = json!({
        "network": network,
        "condition": condition,
        "identity": hex::encode(id),
        "blob": hex::encode(&blob),
        "federation": fed_pub,
        "masterPub": hex_canonical(&fed.mpk),
        "partials": partials,
        "dId": hex_canonical(&d_id),
        "gatedEnvelope": env,
    });
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
}
