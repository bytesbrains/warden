//! `warden-dealer` — the trusted-dealer ceremony (Phase 0 PoC, Maktub issue #181 WS-B).
//!
//! Generates the federation master secret, Shamir-splits it into `n` per-node shares with
//! threshold `t`, and writes:
//!   <out>/federation.json        — PUBLIC: master public key + every share public key.
//!   <out>/shares/node-<i>.json   — SECRET: one node's share (mode 0600 on unix).
//!
//! The full master secret exists only transiently inside [`warden_core::dealer::deal`] and
//! is dropped (best-effort scrubbed) before any file is written — it is **never persisted**.
//! This is **testnet-only** scaffolding: a single machine materializes the master key. The
//! mainnet path is a real DKG where the master secret is never assembled. See
//! `warden/docs/03-protocol.md`.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rand::rngs::OsRng;
use warden_core::dealer::deal;
use warden_core::fed::{FederationPublic, NodeShareFile};

const DEFAULT_NETWORK: &str = "warden-poc-local";
const DEFAULT_T: usize = 3;
const DEFAULT_N: usize = 5;
const DEFAULT_OUT: &str = "fed";

struct Args {
    network: String,
    t: usize,
    n: usize,
    out: PathBuf,
    force: bool,
}

const USAGE: &str = "\
warden-dealer — trusted-dealer ceremony (testnet only; real DKG replaces it for mainnet)

USAGE:
    warden-dealer [OPTIONS]

OPTIONS:
    --network <NAME>     Federation label (bound into the envelope AEAD)  [default: warden-poc-local]
    -t, --threshold <T>  Threshold: partials from T nodes reconstruct     [default: 3]
    -n, --nodes <N>      Federation size                                  [default: 5]
    --out <DIR>          Output directory                                 [default: fed]
    --force              Overwrite <DIR> if it already exists
    -h, --help           Print this help

OUTPUT:
    <out>/federation.json       PUBLIC  — master public key + share public keys (give to clients)
    <out>/shares/node-<i>.json  SECRET  — one node's share (give to node i only)";

fn parse_args() -> Result<Args, String> {
    let mut a = Args {
        network: DEFAULT_NETWORK.to_string(),
        t: DEFAULT_T,
        n: DEFAULT_N,
        out: PathBuf::from(DEFAULT_OUT),
        force: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut take = |name: &str| -> Result<String, String> {
            it.next().ok_or_else(|| format!("{name} requires a value"))
        };
        match arg.as_str() {
            "--network" => a.network = take("--network")?,
            "-t" | "--threshold" => {
                a.t = take("--threshold")?
                    .parse()
                    .map_err(|_| "--threshold must be a positive integer".to_string())?
            }
            "-n" | "--nodes" => {
                a.n = take("--nodes")?
                    .parse()
                    .map_err(|_| "--nodes must be a positive integer".to_string())?
            }
            "--out" => a.out = PathBuf::from(take("--out")?),
            "--force" => a.force = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            other => return Err(format!("unexpected argument: {other}\n\n{USAGE}")),
        }
    }
    if a.t < 1 || a.t > a.n {
        return Err(format!(
            "invalid threshold: require 1 <= t <= n (t={}, n={})",
            a.t, a.n
        ));
    }
    Ok(a)
}

#[cfg(unix)]
fn write_secret(path: &Path, contents: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600) // owner read/write only — this is secret key material
        .open(path)?;
    f.write_all(contents.as_bytes())
}

#[cfg(not(unix))]
fn write_secret(path: &Path, contents: &str) -> std::io::Result<()> {
    // No POSIX mode bits off-unix; the file still lands in the (ideally restricted) out dir.
    fs::write(path, contents)
}

fn run(args: Args) -> Result<(), Box<dyn Error>> {
    if args.out.exists() {
        if !args.force {
            return Err(format!(
                "output directory {} already exists (refusing to clobber a federation; pass --force to overwrite)",
                args.out.display()
            )
            .into());
        }
        // Footgun guard: `--force` triggers a recursive delete. Refuse a root / top-level
        // path (`/`, an empty path, or any path with no parent) so a stray `--out /` can't
        // wipe the filesystem.
        if args.out.as_os_str().is_empty() || args.out.parent().is_none() {
            return Err(format!(
                "refusing to --force-delete a root or top-level path: {}",
                args.out.display()
            )
            .into());
        }
        fs::remove_dir_all(&args.out)?;
    }
    let shares_dir = args.out.join("shares");
    fs::create_dir_all(&shares_dir)?;

    // The ceremony. `deal` materializes the master secret, splits it, and drops it before
    // returning — only public material + the shares survive.
    let out = deal(args.t, args.n, &mut OsRng)?;

    // Public file.
    let pubf = FederationPublic::new(&args.network, out.t, out.n, &out.mpk, &out.share_pubkeys);
    let public_path = args.out.join("federation.json");
    fs::write(&public_path, serde_json::to_string_pretty(&pubf)? + "\n")?;

    // Per-node secret files (0600).
    let mut share_paths = Vec::with_capacity(out.shares.len());
    for share in &out.shares {
        let nf = NodeShareFile::new(&args.network, out.t, out.n, share, &out.mpk);
        let path = shares_dir.join(format!("node-{}.json", share.index));
        write_secret(&path, &(serde_json::to_string_pretty(&nf)? + "\n"))?;
        share_paths.push(path);
    }

    let fingerprint = &pubf.mpk[..16.min(pubf.mpk.len())];
    println!(
        "Dealt a {}-of-{} federation '{}'.",
        args.t, args.n, args.network
    );
    println!("  master pubkey (fingerprint): {fingerprint}…");
    println!(
        "  PUBLIC  {} (master pubkey + {} share pubkeys)",
        public_path.display(),
        out.n
    );
    for p in &share_paths {
        println!("  SECRET  {}", p.display());
    }
    println!();
    println!("⚠️  Testnet only. The master secret was materialized on this machine; never do");
    println!("    this on mainnet (use a real DKG). Distribute each node-<i>.json to node i");
    println!("    over a secure channel and delete the local copies.");
    Ok(())
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    match run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
