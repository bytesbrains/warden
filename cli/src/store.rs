//! A local content-addressed envelope store. The **CID** is `sha256(envelope)` in hex — a
//! stand-in for an IPFS CID; the same envelope always lands at the same id. (Swapping in a
//! real IPFS/Arweave backend is a later phase; the on-chain footprint is just this id.)

use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use warden_core::envelope::Envelope;

/// Content id of an envelope: `sha256` of its compact JSON, hex-encoded.
pub fn cid(env: &Envelope) -> Result<String, String> {
    let bytes = serde_json::to_vec(env).map_err(|e| e.to_string())?;
    Ok(hex::encode(Sha256::digest(&bytes)))
}

/// Write the envelope to `<store_dir>/<cid>.json`; returns `(cid, path)`.
pub fn put(store_dir: &Path, env: &Envelope) -> Result<(String, PathBuf), String> {
    fs::create_dir_all(store_dir).map_err(|e| format!("creating {}: {e}", store_dir.display()))?;
    let cid = cid(env)?;
    let path = store_dir.join(format!("{cid}.json"));
    let pretty = serde_json::to_string_pretty(env).map_err(|e| e.to_string())? + "\n";
    fs::write(&path, pretty).map_err(|e| format!("writing {}: {e}", path.display()))?;
    Ok((cid, path))
}

/// Load an envelope by `reference` — a direct file path if it exists, else a CID looked up
/// in `store_dir`.
pub fn load(store_dir: &Path, reference: &str) -> Result<Envelope, String> {
    let direct = Path::new(reference);
    let path = if direct.is_file() {
        direct.to_path_buf()
    } else {
        store_dir.join(format!("{reference}.json"))
    };
    let raw = fs::read_to_string(&path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    serde_json::from_str(&raw).map_err(|e| format!("parsing envelope {}: {e}", path.display()))
}
