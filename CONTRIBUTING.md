# Contributing to Warden

Warden is a standalone, public-good **threshold conditional-decryption network** —
independent of any single application (Maktub's Veil is one consumer, not the
owner). Contributions that strengthen the cryptography, the node, the clients,
the specs, or the operator experience are very welcome.

> **Status:** Phase 0 proof of concept — pre-audit; the testnet federation is
> all-ours and offers no security. Build and experiment freely, but don't protect
> real secrets with it yet. See [`docs/07-roadmap.md`](docs/07-roadmap.md).

## Layout

A Cargo workspace: `core/` (crypto + envelope + federation format), `dealer/`
(testnet trusted-dealer CLI), `node/` (`wardend` daemon), `cli/` (`warden`
client), `ffi/` + `wasm/` (consumer bindings), `e2e/` (live/local harness). The
authoritative specs live in [`docs/`](docs/00-overview.md); see also `CLAUDE.md`.

## Development

```sh
# rustup honors rust-toolchain.toml (Rust 1.83)
cargo build --release                              # whole workspace
cargo test                                         # offline crypto loop — no chain needed
cargo fmt --check && cargo clippy --all-targets -- -D warnings
```

The `wasm/` crate is a standalone workspace (build with `wasm-pack`). The `e2e/`
harness is Node/ethers — run it from the repo root.

## Ground rules

- **The toolchain (Rust 1.83) and several transitive deps are deliberately pinned**
  for that channel. Don't bump the toolchain or unpin without checking the
  constraints noted in `CLAUDE.md` and `core/Cargo.toml` (newer versions need
  edition 2024 / Rust 1.85).
- **Never commit secret material.** The dealer writes share material to `/fed/`
  and the harness writes scratch under `/e2e/` — both git-ignored. Keep it that
  way: a single committed share compromises a federation.
- **Crypto changes need extra care.** The security-critical paths are `core/`
  (IBE, the envelope) and `node/src/eval.rs` (condition evaluation + finality).
  Explain the security reasoning in the PR, add or update tests, and keep the
  threat model ([`docs/05-threat-model.md`](docs/05-threat-model.md)) honest.
- Run `fmt` + `clippy` (warnings = errors) and the test suite before opening a PR.

## Running a node

Warden's security *is* its independent operators. If you'd run a node in the
federation, see [warden.bytesbrains.com](https://warden.bytesbrains.com) or reach
out at **contact@bytesbrains.com**.

## Security

Found a vulnerability? **Don't open a public issue** — see [`SECURITY.md`](SECURITY.md).

## License

By contributing, you agree that your contributions are licensed under this
repository's **MIT** license.
