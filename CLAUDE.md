# warden/ — Conditional-Decryption Network (Rust)

Scoped context for the Warden workspace — the **Veil** conditional-decryption network (time-bound encrypted delivery). Read alongside the root [`../CLAUDE.md`](../CLAUDE.md). This is early-phase: foundational specs + a crypto core, not yet wired into the live protocol.

## Toolchain

- Rust, pinned to channel **1.83.0** (`core/rust-toolchain.toml`, components `rustfmt` + `clippy`). Edition 2021.
- Crate `warden-core` (`core/Cargo.toml`, v0.0.0). Uses `k256` (ECDH) + `serde`. **Transitive deps are deliberately pinned** to versions compatible with Rust 1.83 (e.g. `base64ct =1.6.0`, `zeroize <1.9`) — newer versions require edition2024 / Rust 1.85. Don't bump the toolchain or unpin without checking those constraints.

## Layout

| Path | Contents |
|---|---|
| `core/` | `warden-core` crate — threshold IBE crypto core, the warden-v1 double-wrap envelope. |
| `docs/` | The authoritative specs — start at [`docs/00-overview.md`](docs/00-overview.md). |
| `README.md`, `core/README.md` | Workspace + crate intros. |

## Specs (read these first)

`docs/`: `00-overview`, `01-architecture`, `02-condition-model`, `03-protocol`, `04-envelope-format`, `05-threat-model`, `06-operator-manual`, `07-roadmap`, `08-architecture-decision`, `GLOSSARY`.

## Conventions & gotchas

- Veil is the long-horizon "time-bound encrypted delivery IS the product" direction — see the foundational specs and DECISION_LOG. It does **not** add governance/upgradeability to the protocol layer; the root immutability invariants still hold.
- Keep `rust-toolchain.toml` and the pinned transitive deps in sync — see the comments in `core/Cargo.toml`.

## Commands (`cd warden/core`)

- `cargo build` — build the crate.
- `cargo test` — run tests.
- `cargo fmt` / `cargo clippy` — format / lint.
