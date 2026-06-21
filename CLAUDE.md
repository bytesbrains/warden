# warden/ — Conditional-Decryption Network (Rust)

Scoped context for the Warden workspace — the **Veil** conditional-decryption network (time-bound encrypted delivery). Read alongside the root [`../CLAUDE.md`](../CLAUDE.md). The **Phase 0 PoC (#181) is code-complete**: foundational specs plus a full workspace (`core`, `dealer`, `node`, `cli`) and a live-chain `e2e/` harness. The crypto loop is proven offline; not yet wired into the live mobile/web product, and the live Base Sepolia run is operator-driven.

## Toolchain

- Rust, pinned to channel **1.83.0** (`rust-toolchain.toml` at the workspace root, components `rustfmt` + `clippy`). Edition 2021.
- A Cargo **workspace** (`Cargo.toml`, `resolver = "2"`) with one `Cargo.lock` at the root. **Transitive deps are deliberately pinned** in `core/Cargo.toml` to versions compatible with Rust 1.83 (e.g. `base64ct =1.6.0`, `zeroize <1.9`) — newer versions require edition2024 / Rust 1.85. Don't bump the toolchain or unpin without checking those constraints. New member crates must stay within the same channel.

## Layout

| Path | Contents |
|---|---|
| `core/` | `warden-core` crate — threshold IBE crypto core, the `warden-v1` double-wrap envelope, the `fed` federation file format. Library only. |
| `dealer/` | `warden-dealer` crate — the trusted-dealer ceremony CLI (WS-B). Materializes the master secret, Shamir-splits it, writes `federation.json` (public) + `shares/node-<i>.json` (secret, 0600). Testnet only. |
| `node/` | `warden-node` crate (`wardend`, WS-C) — the node daemon: condition-watcher + `POST /partial` threshold release. Reads Base Sepolia at the `finalized` tag (`tiny_http` + `ureq`). The security-critical evaluator is `node/src/eval.rs`. |
| `cli/` | `warden-cli` crate (`warden`, WS-D) — the client: `keygen` / `encrypt` (double-wrap → CID store) / `decrypt` (poll federation → combine → open, retry-until-released). |
| `ffi/` | `warden-ffi` — thin **C-ABI** over core for the Flutter app via `dart:ffi` (Veil): `warden_condition_identity` / `warden_seal_gated` / `warden_open_gated` / `warden_combine` (+ `warden_string_free`). JSON/hex strings in, `{ok,value}` JSON out; panics caught at the boundary. `cdylib`/`staticlib` for Android/iOS. **`ffi/build-mobile.sh [ios\|android\|all] [--out <dir>]`** cross-compiles into `dist/mobile/` by default (override with `--out`; iOS xcframework + 4-ABI Android jniLibs; artifacts git-ignored). |
| `wasm/` | `warden-wasm` — `wasm-bindgen` bindings over core for the TS SDK (Veil): `condition_identity` / `seal_gated` / `open_gated` / `combine`. **Standalone workspace** (own `[workspace]`) — wasm-only + `getrandom`-js, so the parent's host builds/tests ignore it. Compiles to `wasm32`; build the npm pkg with `wasm-pack`. |
| `Dockerfile`, `docker-compose.yml` | Build `wardend` + bring up a 3-node PoC federation. |
| `e2e/` | Veil end-to-end harness (WS-E) — Node/ethers v6 orchestrator. `e2e/veil-e2e.mjs` drives the live Base Sepolia loop; `e2e/local/run.mjs` drives a **local Hardhat devnet** (no funds, `evm_increaseTime` skips the timer) and is the "prove Warden works for all conditions" gate. Not Rust; run from the repo root. Finality/reorg notes in [`e2e/README.md`](e2e/README.md); local runbook in [`e2e/local/README.md`](e2e/local/README.md). |
| `docs/` | The authoritative specs — start at [`docs/00-overview.md`](docs/00-overview.md). |
| `README.md`, `core/README.md`, `node/README.md`, `cli/README.md`, `e2e/README.md` | Workspace + crate + harness intros. |

All five Phase-0 workstreams (WS-A…WS-E, #181) are now in the tree.

**Transitive-dep pins (node + cli):** `idna_adapter = "=1.1.0"` (via `ureq → url → idna`) — 1.2 requires edition2024 / Rust 1.85. Same discipline as `core/Cargo.toml`; don't unpin without checking the toolchain.

## Specs (read these first)

`docs/`: `00-overview`, `01-architecture`, `02-condition-model`, `03-protocol`, `04-envelope-format`, `05-threat-model`, `06-operator-manual`, `07-roadmap`, `08-architecture-decision`, `GLOSSARY`.

## Conventions & gotchas

- Veil is the long-horizon "time-bound encrypted delivery IS the product" direction — see the foundational specs and DECISION_LOG. It does **not** add governance/upgradeability to the protocol layer; the root immutability invariants still hold.
- Keep `rust-toolchain.toml` and the pinned transitive deps in sync — see the comments in `core/Cargo.toml`.
- **`panic = "unwind"` is an invariant** on every workspace profile (`Cargo.toml`): the `warden-ffi` boundary's `catch_unwind` turns a panic into `{"ok":false,…}` instead of aborting the host app. `panic = "abort"` would crash the mobile app — never set it.
- **Mobile cross-compile** adds *targets* (iOS + Android NDK), not a channel bump — the 1.83 pin holds. Build with `ffi/build-mobile.sh`; outputs are git-ignored (build from source, never commit a binary).

## Commands (`cd warden`)

- `cargo build` / `cargo test` — build / test the whole workspace.
- `cargo test -p warden-core` / `-p warden-dealer` — a single crate.
- `cargo clippy --all-targets -- -D warnings` / `cargo fmt --check` — lint / format gate.
- `cargo build --no-default-features` (in `core/`) — production build (no master-secret / dealer path).
- `cargo run -p warden-dealer -- --help` — the dealer CLI.
