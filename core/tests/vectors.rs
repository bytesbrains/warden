//! Cross-language known-answer vectors (#184).
//!
//! `identity = SHA-256("warden-cond-v1" ‖ jcs(condition))` is the **linchpin** of the whole
//! system: a node releases for an identity, the client encrypts to it, and every port
//! (WASM/SDK, Dart-FFI) must compute it **byte-for-byte identically** — otherwise a payload
//! sealed by one and a condition evaluated by another never line up and nothing decrypts.
//!
//! These vectors pin the canonical JSON (RFC-8785-style: sorted keys, compact) and the
//! resulting 32-byte identity for two reference conditions. The WASM and Dart ports MUST
//! reproduce these exactly. Gate-layer (`warden-gate-v1`) `aad()`/`pad()` byte vectors are
//! added alongside the WASM build (cross-language round-trip), per the #206 review.

use serde_json::json;
use warden_core::condition::{Condition, Meta, Test};

fn contract(chain: u64, address: &str, func: &str, args: &[&str], word: u32) -> Condition {
    Condition::Contract {
        chain,
        address: address.into(),
        func: func.into(),
        args: args.iter().map(|s| s.to_string()).collect(),
        word,
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

/// Single-value getter (the documented generic example; `word` omitted from the canonical form).
#[test]
fn kat_single_value_executed_condition() {
    let c = contract(
        8453,
        "0x00",
        "executed(uint256)",
        &["12345678901234567890"],
        0,
    );
    assert_eq!(
        c.canonical_bytes().unwrap().as_slice(),
        br#"{"address":"0x00","args":["12345678901234567890"],"chain":8453,"fn":"executed(uint256)","meta":{"finality":32,"tier":1},"test":{"cmp":"==","value":true},"type":"contract"}"#
    );
    assert_eq!(
        hex::encode(c.identity().unwrap()),
        "a6eda22ec724c710fc4eed980e4aafc59150713b2cb9e27933cda8963ae05298"
    );
}

/// The real Veil release condition: `MaktubCore.getHeartbeat(beatId).executed == true`
/// (Base Sepolia, `word: 7`). `word` appears in the canonical form because it is non-zero.
#[test]
fn kat_veil_getheartbeat_word7_condition() {
    let c = contract(
        84532,
        "0xb603C96D089F64Ac487EE0bdaE97D49848F86133",
        "getHeartbeat(uint256)",
        &["777"],
        7,
    );
    assert_eq!(
        c.canonical_bytes().unwrap().as_slice(),
        br#"{"address":"0xb603C96D089F64Ac487EE0bdaE97D49848F86133","args":["777"],"chain":84532,"fn":"getHeartbeat(uint256)","meta":{"finality":32,"tier":1},"test":{"cmp":"==","value":true},"type":"contract","word":7}"#
    );
    assert_eq!(
        hex::encode(c.identity().unwrap()),
        "47fce3a147fc844978e8301a7aedbf437100eda9f769ac0d559c85d806cdb68e"
    );
}
