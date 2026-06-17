//! `wardend` — a Warden node (Phase 0 PoC, Maktub issue #181 WS-C).
//!
//! Holds **one** share of the federation master key and answers `POST /partial { condition }`
//! by releasing its threshold-BLS partial **iff** the condition holds against finalized Base
//! Sepolia state. It never sees plaintext and never holds the master key. Config via env
//! (see [`config`]). The security-critical evaluation lives in [`eval`].
//!
//! Endpoints:
//! - `POST /partial` — `{ "condition": {…} }` → release / not-met / rejected (see [`handler`]).
//! - `GET /health` — liveness.
//! - `GET /info` — node index, network, chain, finality tag, master pubkey (no secrets).

mod abi;
mod config;
mod convert;
mod eval;
mod handler;
mod rpc;

use std::io::Read;
use std::process::ExitCode;

use serde_json::json;
use tiny_http::{Header, Method, Request, Response, Server};

use config::{Config, FinalityTag};
use rpc::RpcClient;

/// Maximum `POST /partial` body size (bytes) — a condition is tiny; this just caps abuse.
const MAX_BODY_BYTES: u64 = 1024 * 1024;

fn json_header() -> Header {
    Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
        .expect("static header is valid")
}

fn info_json(cfg: &Config) -> String {
    json!({
        "index": cfg.index,
        "network": cfg.network,
        "chain_id": cfg.chain_id,
        "finality": cfg.finality.as_rpc(),
        "mpk": handler::canon_hex(&cfg.mpk),
    })
    .to_string()
}

/// Map a request to `(status, json_body)`.
fn route(request: &mut Request, cfg: &Config, rpc: &RpcClient) -> (u16, String) {
    match (request.method(), request.url()) {
        (Method::Get, "/health") => (200, json!({"status": "ok"}).to_string()),
        (Method::Get, "/info") => (200, info_json(cfg)),
        (Method::Post, "/partial") => {
            // Cap the body so an oversized payload can't exhaust memory. A condition is a few
            // hundred bytes; 1 MiB is generous. A truncated body just fails to parse → 400.
            let mut body = String::new();
            if request
                .as_reader()
                .take(MAX_BODY_BYTES)
                .read_to_string(&mut body)
                .is_err()
            {
                return (
                    400,
                    json!({"released": false, "reason": "unreadable_body"}).to_string(),
                );
            }
            let r = handler::handle_partial(&body, &cfg.share, rpc, cfg.chain_id, cfg.finality);
            (r.status, r.body)
        }
        _ => (404, json!({"error": "not found"}).to_string()),
    }
}

fn main() -> ExitCode {
    let cfg = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("config error: {e}");
            return ExitCode::FAILURE;
        }
    };
    let rpc = RpcClient::new(&cfg.rpc_url);
    let server = match Server::http(&cfg.listen) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("failed to bind {}: {e}", cfg.listen);
            return ExitCode::FAILURE;
        }
    };

    eprintln!(
        "wardend node {} up (network={}, chain={}, finality={}) on {}",
        cfg.index,
        cfg.network,
        cfg.chain_id,
        cfg.finality.as_rpc(),
        cfg.listen
    );
    if cfg.finality != FinalityTag::Finalized {
        eprintln!(
            "⚠️  finality tag is '{}' — NOT reorg-safe. Use 'finalized' outside local testing.",
            cfg.finality.as_rpc()
        );
    }

    // Single-threaded: partials are microseconds and the PoC client queries sequentially. A
    // thread pool over Arc<Server> is a trivial later step if liveness needs it.
    for mut request in server.incoming_requests() {
        let (status, body) = route(&mut request, &cfg, &rpc);
        let resp = Response::from_string(body)
            .with_status_code(status)
            .with_header(json_header());
        let _ = request.respond(resp);
    }
    ExitCode::SUCCESS
}
