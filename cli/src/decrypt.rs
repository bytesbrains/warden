//! `warden decrypt` — fetch partials from the federation, combine, and open the envelope.
//!
//! ```text
//! warden decrypt --federation fed/federation.json --nodes url1,url2,url3 \
//!   (--key <secret-hex> | --key-file <file>) --envelope <cid|file> \
//!   [--store <dir>] [--timeout <secs>] [--interval <secs>]
//! ```
//! Writes the payload to `--out <file>` or stdout. Idempotent: retries until `t` nodes
//! release (a monotonic condition only ratchets toward true), then combines + opens.

use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use warden_core::envelope::open;
use warden_core::ibe::combine_verified;

use crate::args::Args;
use crate::client::{collect_partials, PollConfig};
use crate::{keys, store, util};

pub fn run(argv: Vec<String>) -> Result<(), String> {
    let a = Args::parse(argv)?;
    let fed = util::load_federation(a.require("federation")?)?;

    // Validate the cheap args (node count, interval) before any further I/O so an
    // unsatisfiable request fails fast instead of polling to the timeout.
    let nodes: Vec<String> = a
        .require("nodes")?
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if nodes.len() < fed.t {
        return Err(format!(
            "only {} node(s) given but the threshold is {} — cannot reach a {}-of-{} combine",
            nodes.len(),
            fed.t,
            fed.t,
            fed.n
        ));
    }
    let interval = secs(&a, "interval", 3);
    if interval == 0 {
        return Err("--interval must be at least 1 second (avoids a busy poll loop)".into());
    }

    let spks = fed.share_public_keys().map_err(|e| e.to_string())?;
    let sk = keys::secret_from_hex(&read_key(&a)?)?;
    let store_dir = PathBuf::from(a.get("store").unwrap_or("store"));
    let env = store::load(&store_dir, a.require("envelope")?)?;
    let id = env.condition.identity().map_err(|e| e.to_string())?;
    let cfg = PollConfig {
        timeout: Duration::from_secs(secs(&a, "timeout", 120)),
        interval: Duration::from_secs(interval),
    };
    eprintln!(
        "polling {} node(s) for {}-of-{} release of {}…",
        nodes.len(),
        fed.t,
        fed.n,
        hex::encode(id)
    );

    let partials = collect_partials(&nodes, &env.condition, fed.t, &spks, &id, &cfg)
        .map_err(|e| e.to_string())?;
    let d_id = combine_verified(&partials, &id, &spks).map_err(|e| format!("combine: {e}"))?;
    let payload = open(&env, &d_id, &sk).map_err(|e| format!("open: {e}"))?;

    match a.get("out") {
        Some(out) => {
            std::fs::write(out, &payload).map_err(|e| format!("writing {out}: {e}"))?;
            eprintln!("recovered {} bytes → {out}", payload.len());
        }
        None => std::io::stdout()
            .write_all(&payload)
            .map_err(|e| e.to_string())?,
    }
    Ok(())
}

fn read_key(a: &Args) -> Result<String, String> {
    if let Some(hexkey) = a.get("key") {
        return Ok(hexkey.to_string());
    }
    let path = a
        .require("key-file")
        .map_err(|_| "provide --key <hex> or --key-file <file>".to_string())?;
    std::fs::read_to_string(path).map_err(|e| format!("reading {path}: {e}"))
}

fn secs(a: &Args, key: &str, default: u64) -> u64 {
    a.get(key).and_then(|s| s.parse().ok()).unwrap_or(default)
}
