//! WASM bindings over `warden-core` for the TypeScript SDK (Veil).
//!
//! Exposes exactly the four operations the SDK needs — all JSON/hex in, JSON/hex out — so the
//! pairing-based crypto lives once in audited Rust and the SDK never reimplements it:
//! - [`condition_identity`] — `H(condition)` (the cross-language linchpin; KATs in `core/tests/vectors.rs`).
//! - [`seal_gated`] — wrap an already-encrypted blob in the `warden-gate-v1` condition gate.
//! - [`open_gated`] — undo the gate given the released key `d_id`.
//! - [`combine`] — verify + Lagrange-combine node partials into `d_id`.
//!
//! ⚠️ Not audited. PoC.

use std::collections::BTreeMap;

use ark_bls12_381::G1Projective;
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use rand::rngs::OsRng;
use wasm_bindgen::prelude::*;

use warden_core::condition::Condition;
use warden_core::envelope::{self, GatedEnvelope};
use warden_core::fed::FederationPublic;
use warden_core::ibe::{combine_verified, verify_partial, MasterPublicKey, Partial};

fn err<E: std::fmt::Display>(e: E) -> JsError {
    JsError::new(&e.to_string())
}

fn de_hex<T: CanonicalDeserialize>(hexstr: &str, what: &str) -> Result<T, JsError> {
    let bytes = hex::decode(hexstr).map_err(err)?;
    T::deserialize_compressed(bytes.as_slice()).map_err(|_| JsError::new(&format!("invalid {what}")))
}

fn to_hex<T: CanonicalSerialize>(v: &T) -> String {
    let mut buf = Vec::new();
    v.serialize_compressed(&mut buf).expect("serialize to Vec");
    hex::encode(buf)
}

/// `H("warden-cond-v1" ‖ jcs(condition))` as 32-byte hex. Must match the Rust KATs.
#[wasm_bindgen]
pub fn condition_identity(condition_json: &str) -> Result<String, JsError> {
    let cond: Condition = serde_json::from_str(condition_json).map_err(err)?;
    Ok(hex::encode(cond.identity().map_err(err)?))
}

/// Gate an already-encrypted `blob` (hex) on `condition` under `master_pub` (hex). Returns the
/// `warden-gate-v1` envelope as JSON.
#[wasm_bindgen]
pub fn seal_gated(
    condition_json: &str,
    master_pub_hex: &str,
    network: &str,
    blob_hex: &str,
) -> Result<String, JsError> {
    let cond: Condition = serde_json::from_str(condition_json).map_err(err)?;
    let mpk: MasterPublicKey = de_hex(master_pub_hex, "master public key")?;
    let blob = hex::decode(blob_hex).map_err(err)?;
    let env = envelope::seal_gated(cond, &mpk, network, &blob, &mut OsRng).map_err(err)?;
    serde_json::to_string(&env).map_err(err)
}

/// Open a `warden-gate-v1` envelope (JSON) with the released key `d_id` (hex). Returns the
/// original blob as hex.
#[wasm_bindgen]
pub fn open_gated(envelope_json: &str, d_id_hex: &str) -> Result<String, JsError> {
    let env: GatedEnvelope = serde_json::from_str(envelope_json).map_err(err)?;
    let d_id: G1Projective = de_hex(d_id_hex, "d_id")?;
    Ok(hex::encode(envelope::open_gated(&env, &d_id).map_err(err)?))
}

/// Verify node partials against the federation's share public keys and Lagrange-combine `t`
/// of them into `d_id` (hex). `partials_json` is a JSON array of hex-encoded `Partial`s
/// (collected from any number of nodes — may include duplicates, malformed, or invalid ones);
/// `fed_json` is the public `federation.json`; `id_hex` is the condition identity.
///
/// **Tolerant of a noisy federation:** malformed / wrong-index / signature-invalid partials are
/// dropped (not fatal), and partials are deduped by node index — so a single down or malicious
/// node can't fail or grief the combine. Errors only if fewer than `t` *valid* partials remain.
#[wasm_bindgen]
pub fn combine(partials_json: &str, id_hex: &str, fed_json: &str) -> Result<String, JsError> {
    let hexes: Vec<String> = serde_json::from_str(partials_json).map_err(err)?;
    let id = hex::decode(id_hex).map_err(err)?;
    let fed: FederationPublic = serde_json::from_str(fed_json).map_err(err)?;
    let spks = fed.share_public_keys().map_err(err)?;

    // Keep only partials that verify against their published share pubkey; dedup by index.
    let mut good: BTreeMap<u64, Partial> = BTreeMap::new();
    for h in &hexes {
        let Ok(p) = de_hex::<Partial>(h, "partial") else {
            continue;
        };
        if let Some(spk) = spks.iter().find(|s| s.index == p.index) {
            if verify_partial(&p, &id, spk) {
                good.entry(p.index).or_insert(p);
            }
        }
    }
    if good.len() < fed.t {
        return Err(JsError::new(&format!(
            "only {} valid partials, need t={}",
            good.len(),
            fed.t
        )));
    }
    let chosen: Vec<Partial> = good.into_values().take(fed.t).collect();
    let d_id = combine_verified(&chosen, &id, &spks).map_err(err)?;
    Ok(to_hex(&d_id))
}
