//! Poll the federation for partials and combine them.
//!
//! Each round asks every node `POST /partial { condition }`. A node returns `released:true`
//! with its `partial` (a node that hasn't seen the condition met returns `released:false`, or
//! a transient error). Each partial is **verified** against its share public key before being
//! kept, and deduped by node index — so one malicious/buggy node can't corrupt the combine.
//! The loop retries (idempotent: monotonic conditions only ratchet toward true) until `t`
//! verified partials are collected or the deadline passes.

use std::collections::BTreeMap;
use std::thread::sleep;
use std::time::{Duration, Instant};

use ark_serialize::CanonicalDeserialize;
use serde_json::{json, Value};

use warden_core::condition::Condition;
use warden_core::ibe::{verify_partial, Partial, SharePublicKey};

#[derive(thiserror::Error, Debug)]
pub enum ClientError {
    #[error("timed out: collected {got} of {needed} partials")]
    Timeout { got: usize, needed: usize },
}

pub struct PollConfig {
    pub timeout: Duration,
    pub interval: Duration,
}

/// Collect `t` verified, distinct partials for `condition` (identity `id`) from `nodes`.
pub fn collect_partials(
    nodes: &[String],
    condition: &Condition,
    t: usize,
    spks: &[SharePublicKey],
    id: &[u8; 32],
    cfg: &PollConfig,
) -> Result<Vec<Partial>, ClientError> {
    let body = json!({ "condition": condition }).to_string();
    let deadline = Instant::now() + cfg.timeout;
    let mut have: BTreeMap<u64, Partial> = BTreeMap::new();

    loop {
        for node in nodes {
            if have.len() >= t {
                break;
            }
            if let Some(p) = try_node(node, &body, id, spks) {
                have.entry(p.index).or_insert(p);
            }
        }
        if have.len() >= t {
            return Ok(have.into_values().collect());
        }
        if Instant::now() >= deadline {
            return Err(ClientError::Timeout {
                got: have.len(),
                needed: t,
            });
        }
        sleep(cfg.interval);
    }
}

/// Query one node; return its partial only if released, decodable, and **verified** against
/// its share public key. Any failure (offline, not-met, bad partial) → `None` (retry later).
fn try_node(node: &str, body: &str, id: &[u8; 32], spks: &[SharePublicKey]) -> Option<Partial> {
    let url = format!("{}/partial", node.trim_end_matches('/'));
    // Per-request timeout so one hung node can't stall the whole poll past `cfg.timeout`.
    let resp = ureq::post(&url)
        .timeout(Duration::from_secs(5))
        .set("Content-Type", "application/json")
        .send_string(body)
        .ok()?;
    let v: Value = resp.into_json().ok()?;
    if !v.get("released").and_then(Value::as_bool).unwrap_or(false) {
        return None;
    }
    let bytes = hex::decode(v.get("partial")?.as_str()?).ok()?;
    let p = Partial::deserialize_compressed(bytes.as_slice()).ok()?;
    let Some(spk) = spks.iter().find(|s| s.index == p.index) else {
        eprintln!(
            "warning: {node} returned a partial with unknown share index {}",
            p.index
        );
        return None;
    };
    if !verify_partial(&p, id, spk) {
        eprintln!(
            "warning: {node} returned an INVALID partial (index {}) — ignoring",
            p.index
        );
        return None;
    }
    Some(p)
}
