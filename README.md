# Warden

Site: [warden.bytesbrains.com](https://warden.bytesbrains.com)

**Warden** is a drand-derived, **event-gated threshold conditional-decryption network**: a federation of independent nodes that hold shares of one master key (via DKG) and release a per-item decryption key **only when an on-chain condition becomes true** (e.g. `MaktubCore.executed(beatId) == true` on Base).

Warden is a **standalone public good**: any application needing "decrypt only when this on-chain condition holds" can build on it. One such consumer is **Veil**, Maktub's time-bound, revocable, end-to-end-sealed delivery layer — Veil seals the letter and uses Warden to keep the key until the trigger condition fires, then release it.

> *"The app seals the letter; Warden keeps the key until the on-chain moment, then releases it."*

## Status

**Phase 0 PoC — code-complete**: the crypto core, double-wrap envelope, trusted-dealer CLI, node + condition-watcher, client CLI, and an end-to-end Base Sepolia harness are all built and merged. The crypto loop is proven offline (`cli/tests/cli_flow.rs`) and the full loop has been run end-to-end on live Base Sepolia. Next: public testnet, then open-source + audit, then a mainnet federation. See [`docs/07-roadmap.md`](docs/07-roadmap.md). **All-ours testnet = zero security by design; do not use for real secrets.**

## What it is / is not

- **Is:** an off-chain threshold-IBE key-release network. Reuses drand's cryptography (threshold BLS on BLS12-381, Boneh–Franklin IBE / tlock, DKG, resharing); replaces drand's *time/round* trigger with an *on-chain-condition* trigger.
- **Is not:** a blockchain, a token, a storage layer, or a custodian of plaintext. Warden never sees plaintext (content stays end-to-end encrypted to the recipient — see the double-wrap in [`docs/01-architecture.md`](docs/01-architecture.md)). Warden gates *timing*, not *content*.

## Repository layout

A Cargo workspace (`Cargo.toml`) plus the specs. Toolchain pinned to Rust 1.83 (`rust-toolchain.toml`).

| Path | Purpose |
|---|---|
| `core/` | `warden-core` crate — condition model, threshold IBE, the `warden-v1` double-wrap envelope, federation file format |
| `dealer/` | `warden-dealer` crate — trusted-dealer ceremony CLI (testnet only; real DKG for mainnet) |
| `node/` | `warden-node` crate (`wardend`) — node daemon: condition-watcher + threshold partial release over HTTP |
| `cli/` | `warden-cli` crate (`warden`) — client: keygen, encrypt (double-wrap → CID), decrypt (poll → combine → open) |
| `ffi/` | `warden-ffi` — C-ABI over core for Flutter/`dart:ffi` consumers; cdylib/staticlib for Android/iOS |
| `wasm/` | `warden-wasm` — wasm-bindgen bindings over core for TS/JS consumers; standalone workspace, compiles to `wasm32` |
| `e2e/` | End-to-end harness — drives the live Base Sepolia loop (create → seal → execute → decrypt → deactivate); finality/reorg notes in [`e2e/README.md`](e2e/README.md) |
| `Dockerfile`, `docker-compose.yml` | Build `wardend`; bring up a 3-node PoC federation |
| `docs/00-overview.md` | What Warden is, goals, non-goals |
| `docs/01-architecture.md` | System architecture; drand reuse/replace |
| `docs/02-condition-model.md` | The general condition spec (identity = `H(condition)`) |
| `docs/03-protocol.md` | DKG, threshold release, resharing, finality |
| `docs/04-envelope-format.md` | The ciphertext envelope (`warden-v1`) |
| `docs/05-threat-model.md` | Trust model, weaknesses, tiers |
| `docs/06-operator-manual.md` | Node requirements + partner onboarding |
| `docs/07-roadmap.md` | Phased plan (PoC → testnet → open → mainnet) |
| `docs/GLOSSARY.md` | Terms (DKG, IBE, threshold BLS, …) |
| `docs/whitepaper/` | 📄 **Whitepaper** — design, threat model & early deployment ([`whitepaper.md`](docs/whitepaper/whitepaper.md)) |
| `docs/references/drand-analysis.md` | Fetched + analyzed drand v2 reference |

## Build & run

Rust toolchain is pinned (`rust-toolchain.toml`); `rustup` will honor it automatically.

```sh
# Build the whole workspace (node, dealer, cli, core)
cargo build --release

# Run the test suite (offline crypto loop — no chain needed)
cargo test

# Bring up a local 3-node PoC federation (2-of-3) against Base Sepolia:
cargo run -p warden-dealer -- --out fed -n 3 -t 2 --network warden-poc-local
export WARDEN_RPC_URL=https://sepolia.base.org   # or your own endpoint
docker compose up --build
```

`wardend` (the node) and `warden` (the client) are the two binaries; see [`docs/06-operator-manual.md`](docs/06-operator-manual.md) to run a node and [`cli/`](cli) for the client flow. A prebuilt `wardend` image is published at **[`bytesbrains/warden`](https://hub.docker.com/r/bytesbrains/warden)** on Docker Hub (`docker pull bytesbrains/warden`), so operators can run a node without building from source. **All-ours testnet = zero security by design; do not use for real secrets.**

### Client bindings (for consuming apps)

A consuming app embeds one of two bindings over the same Rust core (Maktub's **Veil** layer is one such consumer):

- **Flutter / Dart** — the `warden_ffi` binding is published on **[pub.dev](https://pub.dev/packages/warden_ffi)**. The native library it wraps is published as **GitHub Release assets** (`WardenFfi.xcframework.zip` + `warden-ffi-android-jniLibs.zip`, built by the `mobile-release` workflow on each `v*` tag), so consumers download a prebuilt binary instead of needing a Rust cross-compile toolchain. Building from source is still available for warden developers via `ffi/build-mobile.sh [ios|android|all] [--out <dir>]`. See [`ffi/README.md`](ffi/README.md) for both paths. _(A turn-key `warden_ffi_flutter` plugin that pulls the release binary automatically is tracked in [#4](https://github.com/bytesbrains/warden/issues/4).)_
- **TypeScript / JavaScript** — build the wasm bindings with `wasm-pack build` in `wasm/`. (Today these are consumed bundled inside a host SDK rather than published as a standalone npm package.)

## Standalone by design

Warden is intentionally **standalone**: any application needing "decrypt only when this on-chain condition holds" can build on it. It originated as the network beneath Maktub's **Veil** delivery layer, and Veil remains a reference consumer, but Warden is a general-purpose primitive — nothing in the core ties it to a single application.

## License

**MIT** — see [`LICENSE`](LICENSE). Chosen so Warden can serve as a public-good network anyone can run.
