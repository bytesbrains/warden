//! Public-API end-to-end for the `warden-v1` envelope (WS-D): a Veil beat sealed and
//! opened entirely through the crate's public surface. Requires `trusted-dealer` (default).
#![cfg(feature = "trusted-dealer")]

use ark_std::rand::{rngs::StdRng, SeedableRng};
use serde_json::json;
use warden_core::condition::{Condition, Meta, Test};
use warden_core::dealer::deal;
use warden_core::ecies::SecretKey;
use warden_core::envelope::{open, seal, Envelope, EnvelopeError};
use warden_core::ibe::{combine_verified, partial};

fn beat(beat_id: &str) -> Condition {
    Condition::Contract {
        chain: 8453,
        address: "0x000000000000000000000000000000000000dead".into(),
        func: "executed(uint256)".into(),
        args: vec![beat_id.into()],
        test: Test {
            cmp: "==".into(),
            value: json!(true),
        },
        meta: Meta {
            finality: 32,
            tier: 1,
        },
    }
}

#[test]
fn veil_beat_seal_release_open() {
    let mut rng = StdRng::seed_from_u64(0xBEA7);
    let fed = deal(3, 5, &mut rng).unwrap();
    let heir = SecretKey::random(&mut rng);

    let cond = beat("777");
    let payload = b"abandon abandon abandon ... about  (a 24-word seed)";

    // Owner seals to the condition + the heir's public key.
    let env: Envelope = seal(
        cond.clone(),
        &heir.public_key(),
        &fed.mpk,
        "warden-testnet-0",
        payload,
        &mut rng,
    )
    .unwrap();

    // Travels as JSON.
    let wire = serde_json::to_string(&env).unwrap();
    let env: Envelope = serde_json::from_str(&wire).unwrap();
    assert_eq!(env.alg, "warden-v1");

    // Owner goes silent → beat executes → federation releases (verified) → heir opens.
    let id = cond.identity().unwrap();
    let partials: Vec<_> = [0, 2, 4]
        .iter()
        .map(|&i| partial(&fed.shares[i], &id))
        .collect();
    let d_id = combine_verified(&partials, &id, &fed.share_pubkeys).unwrap();
    assert_eq!(open(&env, &d_id, &heir).unwrap(), payload);

    // Before the beat executes there is no released key — and a key for any other beat
    // does not release this one (condition gate).
    let other = beat("778").identity().unwrap();
    let op: Vec<_> = [0, 1, 2]
        .iter()
        .map(|&i| partial(&fed.shares[i], &other))
        .collect();
    let wrong = combine_verified(&op, &other, &fed.share_pubkeys).unwrap();
    assert!(matches!(
        open(&env, &wrong, &heir),
        Err(EnvelopeError::NotReleased)
    ));
}
