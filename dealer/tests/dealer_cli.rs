//! Integration test for the `warden-dealer` binary (WS-B acceptance): run the actual CLI,
//! load the files it writes, and prove they drive a full Veil round-trip — seal an
//! envelope against the written master public key, release `t` partials from the written
//! per-node shares, combine against the written share public keys, and open.
//!
//! This is the cross-tool contract the node (WS-C) and client (WS-D) will rely on: the
//! dealer's output is *usable* federation material, not just well-formed JSON.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use rand::rngs::OsRng;
use serde_json::json;
use warden_core::condition::{Condition, Meta, Test};
use warden_core::envelope::{open, seal};
use warden_core::fed::{FederationPublic, NodeShareFile};
use warden_core::ibe::{combine_verified, partial};

const BIN: &str = env!("CARGO_BIN_EXE_warden-dealer");

/// A unique scratch dir for this test process (no temp-dir crate dependency).
fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("warden-dealer-it-{}-{tag}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
}

fn beat(beat_id: &str) -> Condition {
    Condition::Contract {
        chain: 84532,
        address: "0x000000000000000000000000000000000000dead".into(),
        func: "executed(uint256)".into(),
        args: vec![beat_id.into()],
        word: 0,
        test: Test {
            cmp: "==".into(),
            value: json!(true),
        },
        meta: Meta {
            finality: 32,
            tier: 1,
        },
    }
}

#[test]
fn cli_output_drives_a_full_veil_round_trip() {
    let out = scratch("roundtrip");
    let network = "warden-it-net";

    let status = Command::new(BIN)
        .args([
            "--network",
            network,
            "-t",
            "3",
            "-n",
            "5",
            "--out",
            out.to_str().unwrap(),
        ])
        .status()
        .expect("run warden-dealer");
    assert!(status.success(), "dealer should exit 0");

    // Load the public file (what a client gets) + the per-node secret files (what nodes get).
    let pubf: FederationPublic =
        serde_json::from_str(&fs::read_to_string(out.join("federation.json")).unwrap()).unwrap();
    assert_eq!(pubf.network, network);
    assert_eq!((pubf.t, pubf.n), (3, 5));
    assert_eq!(pubf.share_pubkeys.len(), 5);

    let node_files: Vec<NodeShareFile> = (1..=5)
        .map(|i| {
            let p = out.join(format!("shares/node-{i}.json"));
            serde_json::from_str(&fs::read_to_string(p).unwrap()).unwrap()
        })
        .collect();

    let mpk = pubf.master_public_key().unwrap();
    let share_pubkeys = pubf.share_public_keys().unwrap();

    // Owner seals to the condition + an heir key, using ONLY the master pubkey from file.
    let mut rng = OsRng;
    let heir = warden_core::ecies::SecretKey::random(&mut rng);
    let cond = beat("424242");
    let payload = b"the twelve words go here";
    let env = seal(
        cond.clone(),
        &heir.public_key(),
        &mpk,
        network,
        payload,
        &mut rng,
    )
    .unwrap();

    // Three nodes release partials from their loaded shares; client combines (verified
    // against the published share pubkeys) and opens.
    let id = cond.identity().unwrap();
    let partials: Vec<_> = [0usize, 2, 4]
        .iter()
        .map(|&i| partial(&node_files[i].share().unwrap(), &id))
        .collect();
    let d_id = combine_verified(&partials, &id, &share_pubkeys).unwrap();
    assert_eq!(open(&env, &d_id, &heir).unwrap(), payload);

    // Every node file carries the same master pubkey as the public file.
    for nf in &node_files {
        assert_eq!(nf.master_public_key().unwrap(), mpk);
    }

    fs::remove_dir_all(&out).ok();
}

#[test]
fn refuses_to_clobber_without_force() {
    let out = scratch("clobber");
    let args = ["-t", "2", "-n", "3", "--out", out.to_str().unwrap()];

    assert!(Command::new(BIN).args(args).status().unwrap().success());
    // Second run without --force must fail (don't silently overwrite a live federation).
    assert!(!Command::new(BIN).args(args).status().unwrap().success());
    // With --force it succeeds.
    assert!(Command::new(BIN)
        .args(args)
        .arg("--force")
        .status()
        .unwrap()
        .success());

    fs::remove_dir_all(&out).ok();
}

#[test]
fn rejects_bad_threshold() {
    let out = scratch("badthreshold");
    // t > n must be rejected before any files are written.
    let status = Command::new(BIN)
        .args(["-t", "9", "-n", "5", "--out", out.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(!status.success());
    assert!(!out.exists(), "no output dir on invalid params");
}
