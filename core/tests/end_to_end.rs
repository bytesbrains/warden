//! End-to-end WS-A loop: a real Veil condition → identity → trusted-dealer federation →
//! threshold IBE encrypt / **verified** release / decrypt. This is the crypto core of a Veil
//! beat (minus the inner ECIES/AEAD double-wrap, which composes on top — WS-D).
//!
//! Requires the `trusted-dealer` feature (default).
#![cfg(feature = "trusted-dealer")]

use ark_std::rand::{rngs::StdRng, Rng, SeedableRng};
use serde_json::json;
use warden_core::condition::{Condition, Meta, Test};
use warden_core::dealer::deal;
use warden_core::ibe::{combine_verified, decrypt, encrypt, partial, Block, IbeError};

fn beat_condition(beat_id: &str) -> Condition {
    Condition::Contract {
        chain: 8453,
        address: "0x000000000000000000000000000000000000dead".into(),
        func: "executed(uint256)".into(),
        args: vec![beat_id.into()], // uint256 as a decimal STRING
        word: 0,
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
fn condition_gated_threshold_ibe_end_to_end() {
    let mut rng = StdRng::seed_from_u64(0xC0FFEE);

    // 3-of-5 federation via trusted dealer (testnet).
    let fed = deal(3, 5, &mut rng).unwrap();

    // Owner seals a content key to the condition `executed(42)==true`.
    let cond = beat_condition("42");
    cond.validate().unwrap();
    let id = cond.identity().unwrap();
    let content_key: Block = rng.gen();
    let ct = encrypt(&fed.mpk, &id, &content_key, &mut rng);

    // Condition holds → 3 nodes release partials → VERIFY against share pubkeys → combine → decrypt.
    let partials: Vec<_> = [&fed.shares[0], &fed.shares[2], &fed.shares[4]]
        .iter()
        .map(|s| partial(s, &id))
        .collect();
    let d_id = combine_verified(&partials, &id, &fed.share_pubkeys).unwrap();
    assert_eq!(decrypt(&d_id, &ct), Some(content_key));

    // A faulty/malicious partial is attributed to its node, not a silent failure.
    let mut bad = partials.clone();
    bad[0].value = partial(&fed.shares[1], &id).value; // node 1's value under node-index of shares[0]
    assert!(matches!(
        combine_verified(&bad, &id, &fed.share_pubkeys),
        Err(IbeError::InvalidPartial(_))
    ));

    // A key released for a DIFFERENT beat (43) must not open this ciphertext
    // (identity domain-separation — anti cross-beat reuse).
    let other_id = beat_condition("43").identity().unwrap();
    let other_partials: Vec<_> = [&fed.shares[0], &fed.shares[1], &fed.shares[2]]
        .iter()
        .map(|s| partial(s, &other_id))
        .collect();
    let other_key = combine_verified(&other_partials, &other_id, &fed.share_pubkeys).unwrap();
    assert_ne!(decrypt(&other_key, &ct), Some(content_key));
}
