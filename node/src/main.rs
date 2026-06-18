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

use std::collections::HashMap;
use std::io::Read;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use serde_json::json;
use tiny_http::{Header, Method, Request, Response, Server};

use config::{Config, FinalityTag};
use rpc::RpcClient;

/// Maximum `POST /partial` body size (bytes) — a condition is a few hundred bytes; this caps abuse.
const MAX_BODY_BYTES: u64 = 16 * 1024;

/// Per-IP rate limit on `POST /partial` (the only RPC-backed, abusable route). The Veil client
/// polls until release (~1 req / 2s), so a legit client does ~30/min; this window is generous for
/// that while shedding floods. `/health` (fly's frequent liveness probe) and `/info` are not
/// limited. Spam can only waste RPC/CPU — it can never leak: a node releases a partial only for a
/// SATISFIED on-chain condition, regardless of who or how often someone asks.
const RATE_WINDOW: Duration = Duration::from_secs(60);
const RATE_MAX_PER_WINDOW: u32 = 120;
/// Bound the rate table so a flood of distinct IPs can't grow memory unbounded.
const RATE_TABLE_CAP: usize = 50_000;

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

/// Permissive CORS. A Warden node serves ANY client (recipient apps, the browser SDK, the CLI);
/// its security is the on-chain condition check, not the request origin — so `*` is correct, and
/// it lets browser clients reach `/partial`. CORS is not (and must not be relied on as) an abuse
/// control here; rate limiting is.
fn cors_headers() -> [Header; 4] {
    [
        Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap(),
        Header::from_bytes(&b"Access-Control-Allow-Methods"[..], &b"GET, POST, OPTIONS"[..]).unwrap(),
        Header::from_bytes(&b"Access-Control-Allow-Headers"[..], &b"Content-Type"[..]).unwrap(),
        Header::from_bytes(&b"Access-Control-Max-Age"[..], &b"86400"[..]).unwrap(),
    ]
}

/// The client IP for rate limiting. On fly, the edge proxy sets `Fly-Client-IP` (trustworthy).
/// We deliberately do NOT trust `X-Forwarded-For` (client-spoofable → would defeat per-IP
/// limiting). Off-fly / local, fall back to the socket peer.
fn client_ip(request: &Request) -> String {
    for h in request.headers() {
        if h.field.equiv("Fly-Client-IP") {
            let v = h.value.as_str();
            if !v.is_empty() {
                return v.to_string();
            }
        }
    }
    request
        .remote_addr()
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Fixed-window per-IP limiter. The server loop is single-threaded, so no locking is needed.
struct RateLimiter {
    hits: HashMap<String, (u32, Instant)>,
}
impl RateLimiter {
    fn new() -> Self {
        Self { hits: HashMap::new() }
    }
    /// `true` if allowed; `false` once `ip` exceeds `RATE_MAX_PER_WINDOW` within `RATE_WINDOW`.
    fn allow(&mut self, ip: &str, now: Instant) -> bool {
        if self.hits.len() > RATE_TABLE_CAP {
            self.hits
                .retain(|_, (_, start)| now.duration_since(*start) <= RATE_WINDOW);
        }
        let e = self.hits.entry(ip.to_owned()).or_insert((0, now));
        if now.duration_since(e.1) > RATE_WINDOW {
            *e = (0, now);
        }
        e.0 += 1;
        e.0 <= RATE_MAX_PER_WINDOW
    }
}

/// Map a request to `(status, json_body)`.
fn route(
    request: &mut Request,
    cfg: &Config,
    rpc: &RpcClient,
    limiter: &mut RateLimiter,
    ip: &str,
) -> (u16, String) {
    match (request.method(), request.url()) {
        (Method::Options, _) => (204, String::new()), // CORS preflight
        (Method::Get, "/health") => (200, json!({"status": "ok"}).to_string()),
        (Method::Get, "/info") => (200, info_json(cfg)),
        (Method::Post, "/partial") => {
            // Shed floods BEFORE any body read or RPC work.
            if !limiter.allow(ip, Instant::now()) {
                return (
                    429,
                    json!({"released": false, "reason": "rate_limited"}).to_string(),
                );
            }
            // Cap the body so an oversized payload can't exhaust memory. A condition is a few
            // hundred bytes; 16 KiB is generous. A truncated body just fails to parse → 400.
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
    let mut limiter = RateLimiter::new();
    for mut request in server.incoming_requests() {
        let ip = client_ip(&request);
        let (status, body) = route(&mut request, &cfg, &rpc, &mut limiter, &ip);
        let mut resp = Response::from_string(body)
            .with_status_code(status)
            .with_header(json_header());
        for h in cors_headers() {
            resp.add_header(h);
        }
        let _ = request.respond(resp);
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limiter_caps_per_ip_then_resets_after_window() {
        let mut rl = RateLimiter::new();
        let t0 = Instant::now();
        // First RATE_MAX_PER_WINDOW are allowed; the next is shed.
        for _ in 0..RATE_MAX_PER_WINDOW {
            assert!(rl.allow("1.2.3.4", t0));
        }
        assert!(!rl.allow("1.2.3.4", t0), "over-quota request must be rejected");
        // A different IP has its own budget.
        assert!(rl.allow("5.6.7.8", t0));
        // After the window elapses, the original IP is allowed again.
        let later = t0 + RATE_WINDOW + Duration::from_secs(1);
        assert!(rl.allow("1.2.3.4", later), "quota must reset after the window");
    }
}
