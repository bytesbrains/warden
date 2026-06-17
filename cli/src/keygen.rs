//! `warden keygen [--out <secret-file>]` — generate a recipient secp256k1 keypair.
//!
//! Prints the public key (give it to whoever seals to you). The secret key is written 0600
//! to `--out` if given, otherwise printed to stdout (PoC — no real secrets).

use crate::args::Args;
use crate::{keys, util};

pub fn run(argv: Vec<String>) -> Result<(), String> {
    let a = Args::parse(argv)?;
    let (sk, pk) = keys::generate();
    let sk_hex = keys::secret_to_hex(&sk);
    let pk_hex = keys::public_to_hex(&pk);

    println!("public {pk_hex}");
    match a.get("out") {
        Some(path) => {
            util::write_secret(path, &format!("{sk_hex}\n"))?;
            eprintln!("secret key written to {path} (0600) — keep it safe");
        }
        None => println!("secret {sk_hex}"),
    }
    Ok(())
}
