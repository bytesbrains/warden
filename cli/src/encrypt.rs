//! `warden encrypt` — double-wrap a payload and publish it to the local CID store.
//!
//! ```text
//! warden encrypt --federation fed/federation.json --recipient <pubkey-hex> \
//!   (--beat <id> [--core <addr>] | --condition <cond.json>) \
//!   (--message <text> | --payload <file>) [--store <dir>] [--finality <n>]
//! ```
//! Prints the CID of the sealed envelope.

use std::path::PathBuf;

use rand::rngs::OsRng;
use warden_core::condition::Condition;
use warden_core::envelope::seal;

use crate::args::Args;
use crate::{condition, keys, store, util};

pub fn run(argv: Vec<String>) -> Result<(), String> {
    let a = Args::parse(argv)?;
    let fed = util::load_federation(a.require("federation")?)?;
    let mpk = fed.master_public_key().map_err(|e| e.to_string())?;
    let recipient = keys::public_from_hex(a.require("recipient")?)?;
    let cond = build_condition(&a)?;
    let payload = read_payload(&a)?;
    let store_dir = PathBuf::from(a.get("store").unwrap_or("store"));

    let mut rng = OsRng;
    let env = seal(cond, &recipient, &mpk, &fed.network, &payload, &mut rng)
        .map_err(|e| format!("seal: {e}"))?;
    let (cid, path) = store::put(&store_dir, &env)?;

    println!("cid {cid}");
    eprintln!(
        "sealed → {} (network={}, {} payload bytes)",
        path.display(),
        fed.network,
        payload.len()
    );
    Ok(())
}

/// `--beat <id>` builds the Veil condition; otherwise `--condition <file>` is parsed as JSON.
fn build_condition(a: &Args) -> Result<Condition, String> {
    if let Some(beat) = a.get("beat") {
        let core = a.get("core").unwrap_or(condition::DEFAULT_CORE);
        let finality = parse_finality(a)?;
        return Ok(condition::beat_executed(core, beat, finality));
    }
    let path = a
        .require("condition")
        .map_err(|_| "provide --beat <id> (Veil) or --condition <file> (arbitrary)".to_string())?;
    let raw = std::fs::read_to_string(path).map_err(|e| format!("reading {path}: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("parsing condition {path}: {e}"))
}

fn parse_finality(a: &Args) -> Result<u64, String> {
    match a.get("finality") {
        Some(s) => s
            .parse()
            .map_err(|_| format!("--finality not a number: {s:?}")),
        None => Ok(32),
    }
}

/// `--message <text>` (utf-8) or `--payload <file>` (raw bytes).
fn read_payload(a: &Args) -> Result<Vec<u8>, String> {
    if let Some(text) = a.get("message") {
        return Ok(text.as_bytes().to_vec());
    }
    let path = a
        .require("payload")
        .map_err(|_| "provide --message <text> or --payload <file>".to_string())?;
    if path == "-" {
        use std::io::Read;
        let mut buf = Vec::new();
        std::io::stdin()
            .read_to_end(&mut buf)
            .map_err(|e| format!("reading stdin: {e}"))?;
        return Ok(buf);
    }
    std::fs::read(path).map_err(|e| format!("reading {path}: {e}"))
}
