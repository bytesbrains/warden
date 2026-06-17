//! `warden` — the Warden client CLI (Phase 0 PoC, Maktub issue #181 WS-D).
//!
//! - `keygen`  — generate a recipient secp256k1 keypair.
//! - `encrypt` — double-wrap a payload to a condition + recipient; publish to the CID store.
//! - `decrypt` — fetch partials from the federation, combine, and open the envelope.
//!
//! ⚠️ Not audited; PoC. See `../docs/05-threat-model.md`.

mod args;
mod client;
mod condition;
mod decrypt;
mod encrypt;
mod keygen;
mod keys;
mod store;
mod util;

use std::process::ExitCode;

const USAGE: &str = "\
warden — Warden client (Phase 0 PoC)

USAGE:
    warden <COMMAND> [OPTIONS]

COMMANDS:
    keygen     Generate a recipient keypair        [--out <secret-file>]
    encrypt    Double-wrap a payload → CID          --federation <f> --recipient <pubhex>
               (--beat <id> [--core <addr>] | --condition <file>)
               (--message <text> | --payload <file>) [--store <dir>] [--finality <n>]
    decrypt    Fetch partials → combine → open      --federation <f> --nodes <u1,u2,…>
               (--key <hex> | --key-file <f>) --envelope <cid|file>
               [--store <dir>] [--timeout <secs>] [--interval <secs>] [--out <file>]";

fn main() -> ExitCode {
    let mut argv = std::env::args().skip(1);
    let sub = argv.next();
    let rest: Vec<String> = argv.collect();

    let result = match sub.as_deref() {
        Some("keygen") => keygen::run(rest),
        Some("encrypt") => encrypt::run(rest),
        Some("decrypt") => decrypt::run(rest),
        Some("-h") | Some("--help") | None => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Some(other) => Err(format!("unknown command: {other}\n\n{USAGE}")),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
