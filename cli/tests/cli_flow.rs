//! End-to-end CLI flow (WS-D acceptance), fully offline: deal a federation in-process, stand
//! up mock nodes that release partials (simulating a met condition), then drive the real
//! `warden` binary `keygen → encrypt → decrypt` and assert the payload round-trips.
//!
//! This proves the client's whole path — seal, publish to the CID store, poll the federation,
//! deserialize + verify partials, combine, and open — through the actual command surface.
//! Live-chain release is exercised by the WS-E harness.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::thread;

use ark_serialize::CanonicalSerialize;
use ark_std::rand::{rngs::StdRng, SeedableRng};
use warden_core::dealer::deal;
use warden_core::fed::FederationPublic;
use warden_core::ibe::partial;
use warden_core::shamir::Share;

const BIN: &str = env!("CARGO_BIN_EXE_warden");

fn scratch() -> PathBuf {
    // Atomic counter so parallel tests in this process never share a directory.
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("warden-cli-it-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn partial_hex(share: &Share, id: &[u8; 32]) -> String {
    let p = partial(share, id);
    let mut buf = Vec::new();
    p.serialize_compressed(&mut buf).unwrap();
    hex::encode(buf)
}

/// A mock Warden node: always "released", returning its verified partial for the posted
/// condition's identity. Returns its base URL.
fn spawn_mock_node(share: Share) -> String {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let url = format!("http://{}", server.server_addr().to_ip().unwrap());
    thread::spawn(move || {
        for mut req in server.incoming_requests() {
            let mut body = String::new();
            req.as_reader().read_to_string(&mut body).ok();
            let v: serde_json::Value = serde_json::from_str(&body).unwrap();
            let cond: warden_core::condition::Condition =
                serde_json::from_value(v["condition"].clone()).unwrap();
            let id = cond.identity().unwrap();
            let resp = serde_json::json!({
                "released": true,
                "index": share.index,
                "partial": partial_hex(&share, &id),
            })
            .to_string();
            req.respond(tiny_http::Response::from_string(resp)).ok();
        }
    });
    url
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(BIN).args(args).output().expect("run warden")
}

#[test]
fn encrypt_then_decrypt_round_trips_through_the_federation() {
    let dir = scratch();
    let store = dir.join("store");

    // Federation (2-of-3), public file written; shares kept in-memory for the mock nodes.
    let fed = deal(2, 3, &mut StdRng::seed_from_u64(99)).unwrap();
    let fed_pub = FederationPublic::new("warden-it", fed.t, fed.n, &fed.mpk, &fed.share_pubkeys);
    let fed_path = dir.join("federation.json");
    fs::write(&fed_path, serde_json::to_string_pretty(&fed_pub).unwrap()).unwrap();

    let nodes: Vec<String> = fed.shares.iter().cloned().map(spawn_mock_node).collect();
    let nodes_arg = nodes.join(",");

    // A condition file (the mock signs its identity regardless of chain).
    let cond_path = dir.join("cond.json");
    fs::write(
        &cond_path,
        r#"{"type":"contract","chain":84532,"address":"0xb603C96D089F64Ac487EE0bdaE97D49848F86133",
            "fn":"getHeartbeat(uint256)","args":["7"],"word":7,
            "test":{"cmp":"==","value":true},"meta":{"finality":32,"tier":1}}"#,
    )
    .unwrap();

    // keygen → recipient public/secret.
    let kg = run(&["keygen"]);
    assert!(kg.status.success());
    let out = String::from_utf8(kg.stdout).unwrap();
    let pubkey = field(&out, "public");
    let secret = field(&out, "secret");

    // encrypt → CID.
    let message = "the twelve words go here";
    let enc = run(&[
        "encrypt",
        "--federation",
        fed_path.to_str().unwrap(),
        "--recipient",
        &pubkey,
        "--condition",
        cond_path.to_str().unwrap(),
        "--message",
        message,
        "--store",
        store.to_str().unwrap(),
    ]);
    assert!(
        enc.status.success(),
        "encrypt failed: {}",
        String::from_utf8_lossy(&enc.stderr)
    );
    let cid = field(&String::from_utf8(enc.stdout).unwrap(), "cid");

    // decrypt → payload (polls the 3 mock nodes, combines 2-of-3, opens).
    let out_file = dir.join("recovered.txt");
    let dec = run(&[
        "decrypt",
        "--federation",
        fed_path.to_str().unwrap(),
        "--nodes",
        &nodes_arg,
        "--key",
        &secret,
        "--envelope",
        &cid,
        "--store",
        store.to_str().unwrap(),
        "--timeout",
        "10",
        "--out",
        out_file.to_str().unwrap(),
    ]);
    assert!(
        dec.status.success(),
        "decrypt failed: {}",
        String::from_utf8_lossy(&dec.stderr)
    );
    assert_eq!(fs::read_to_string(&out_file).unwrap(), message);

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn decrypt_fails_fast_when_fewer_nodes_than_threshold() {
    let dir = scratch();
    let fed = deal(2, 3, &mut StdRng::seed_from_u64(7)).unwrap();
    let fed_pub = FederationPublic::new("warden-it", fed.t, fed.n, &fed.mpk, &fed.share_pubkeys);
    let fed_path = dir.join("federation.json");
    fs::write(&fed_path, serde_json::to_string_pretty(&fed_pub).unwrap()).unwrap();

    // One node, threshold 2 → must error immediately (before touching the store/envelope).
    let dec = run(&[
        "decrypt",
        "--federation",
        fed_path.to_str().unwrap(),
        "--nodes",
        "http://127.0.0.1:1",
        "--key",
        &"00".repeat(32),
        "--envelope",
        "does-not-exist",
        "--store",
        dir.to_str().unwrap(),
        "--timeout",
        "30",
    ]);
    assert!(!dec.status.success());
    let err = String::from_utf8_lossy(&dec.stderr);
    assert!(
        err.contains("threshold"),
        "expected threshold error, got: {err}"
    );

    fs::remove_dir_all(&dir).ok();
}

/// Pull `<key> <value>` from a line of `key value` output.
fn field(out: &str, key: &str) -> String {
    out.lines()
        .find_map(|l| l.strip_prefix(&format!("{key} ")))
        .unwrap_or_else(|| panic!("no `{key}` line in:\n{out}"))
        .trim()
        .to_string()
}
