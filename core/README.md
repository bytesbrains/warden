# warden-core

Core cryptography for **Warden** — the event-gated threshold conditional-decryption network that powers Veil. Phase 0 PoC (Maktub issue #181, WS-A).

> ⚠️ **Not audited. Not for production.** All-ours testnet = zero security by design (see [`../docs/05-threat-model.md`](../docs/05-threat-model.md)).

## Modules

| Module | What it does |
|---|---|
| `condition` | The condition model + **RFC-8785-style canonicalization** + domain-separated `identity = H(condition)`. `uint256` carried as decimal strings (type-enforced). |
| `shamir` | Shamir secret sharing over the BLS12-381 scalar field + Lagrange interpolation at `x=0`. `split` returns `Result`. |
| `ibe` | **Boneh–Franklin IBE over BLS12-381 (tlock-style)** with **threshold** partial-decryption, **partial verification** (`verify_partial` / `combine_verified` — attributes a bad node), Lagrange combine in G1, Fujisaki–Okamoto CCA, and `CanonicalSerialize` wire types. |
| `dealer` | Trusted-dealer setup — **`trusted-dealer` feature** (default), testnet only; emits per-node share public keys. Replaced by real DKG for mainnet. |
| `ecies` | **secp256k1 ECIES** (recipient gate): ECDH + HKDF-SHA256 + ChaCha20-Poly1305. |
| `envelope` | The **`warden-v1` double-wrap** (`seal`/`open`): AEAD content + ECIES recipient gate + threshold-IBE condition gate, JSON wire form. |
| `fed` | The **federation file format** — `FederationPublic` (master pubkey + share pubkeys; published to clients) and `NodeShareFile` (a node's secret share). Crypto carried as hex-of-canonical; readable without `trusted-dealer`. Written by `warden-dealer`. |

## Features

- `trusted-dealer` (default) — the master-secret path (`MasterKey`, `dealer`). **Production / real-DKG builds disable it:** `cargo build --no-default-features` (warning-clean; `MasterKey` and the dealer are excluded).

## The loop (proven end-to-end in `tests/end_to_end.rs`)

```
condition (executed(beatId)==true)  ──H──▶  identity
owner: encrypt(content_key) to identity under master pubkey      ─▶  ciphertext (U,V,W)
condition holds: t-of-n nodes release partials = share·H1(id)    ─▶  combine (Lagrange in G1) = d_id
decrypt(d_id, ciphertext)                                        ─▶  content_key
```

A key released for a *different* condition cannot open the ciphertext (identity domain-separation).

## Build / test

```bash
cargo test                              # core lib + fed round-trip + double-wrap e2e (incl. partial verification, ECIES KDF binding, AAD tamper, padding)
cargo clippy --all-targets -- -D warnings
cargo build --no-default-features       # production build (no master secret) — warning-clean
```

`warden-core` is a member of the `warden/` workspace; the toolchain pin (`rust-toolchain.toml`, **1.83**) and the committed `Cargo.lock` live at the workspace root. Transitive `zeroize`/`zeroize_derive`/`base64ct` are pinned to pre-edition2024 versions. CI runs fmt/clippy/test (`.github/workflows/ci.yml` → `warden-core`).

## Not yet (later WS / phases / tracked from review)

- The node daemon + Base condition-watcher (WS-C) and client CLI (WS-D) — both consume `fed` files; WASM/Dart-FFI client targets later.
- Reconcile `ecies` byte format with Maktub's existing `RecipientRegistry` ECIES before integration.
- **Before mainnet:** real DKG + resharing; **subgroup/point validation at the wire boundary** (deserialize already validates via arkworks `Validate::Yes`, but the node must reject malformed partials/ciphertext); **true secret zeroization** (current `Drop` is best-effort); freeze domain-separation tags + hash-to-curve DST with **cross-language test vectors**; external audit.
