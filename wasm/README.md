# warden-wasm

`wasm-bindgen` bindings over `warden-core`, so the TypeScript SDK can do Veil **without
reimplementing the pairing crypto** — the audited BLS12-381 / threshold-IBE lives once in Rust.

> ⚠️ Not audited. PoC. Veil's *timing* guarantee is zero-security on the all-ours testnet —
> label it "preview" and never claim unreadable-until-trigger (see `../docs/05-threat-model.md`).

## API (all JSON/hex in, JSON/hex out)

| fn | in → out |
|---|---|
| `condition_identity(conditionJson)` | → `H(condition)` 32-byte hex (the cross-language linchpin; KATs in `../core/tests/vectors.rs`) |
| `seal_gated(conditionJson, masterPubHex, network, blobHex)` | → `warden-gate-v1` envelope JSON (wraps an already-encrypted blob, e.g. Maktub's v2 hybrid) |
| `open_gated(envelopeJson, dIdHex)` | → original blob hex |
| `combine(partialsJson, idHex, fedJson)` | → released key `d_id` hex (verifies partials vs the federation share pubkeys, Lagrange-combines) |

## Build

Standalone crate (own `[workspace]`) so the parent `warden/` host builds/tests ignore this
wasm-only, `getrandom`-js crate. **Verified: compiles to `wasm32-unknown-unknown`** (arkworks
pairings + `getrandom` `js` backend; ~1.5 MB pre-`wasm-opt`).

```bash
rustup target add wasm32-unknown-unknown
cargo build --target wasm32-unknown-unknown --release      # compile-check (no JS glue)
wasm-pack build --target bundler --out-dir pkg             # → pkg/ npm package for the SDK
```

**RNG / build target:** `seal_gated` draws from `OsRng` → `getrandom`'s `js` backend → the host's
Web Crypto (`crypto.getRandomValues` / Node webcrypto), a proper CSPRNG. It therefore requires a
**crypto-capable host** — build for `--target web`/`bundler`/`nodejs` so `getrandom` binds; a wrong
target panics at runtime (fail-closed, but a footgun). `Cargo.lock` is committed so the wasm
artifact is reproducibly pinned (extend the workspace `cargo audit` to these deps — #191).

## Cross-language test

`test/fixture.json` is a deterministic Rust-generated gate-layer vector (#184) — a dealt
federation, a sealed `warden-gate-v1` envelope, the node partials, and the combined `d_id`.
Regenerate with `cargo run -p warden-core --example veil_fixture > warden/wasm/test/fixture.json`.

`test/round_trip.cjs` validates the **wasm against that Rust fixture**: `condition_identity`
matches the fixture + the #207 KAT; `combine` reproduces the Rust `d_id` (and tolerates a bogus
partial); `open_gated` recovers the blob; and a wasm `seal_gated`→`open_gated` round-trip holds.

```bash
wasm-pack build --target nodejs --out-dir pkg --release   # needs current-stable Rust for the tooling
node warden/wasm/test/round_trip.cjs                       # → ✓ warden-wasm round-trip OK
```

> Producing `pkg/` needs `wasm-pack`, whose own deps require edition2024 (Rust ≥ 1.85) — so it
> builds with a current-stable toolchain even though the **crate itself stays pinned to 1.83**
> (the parent `warden/` override). The crate's wasm32 compile is verified under 1.83.

## Next

- SDK `veilSeal`/`veilOpen` consume `pkg/`; the app wires the opt-in (preview) Beat flow.
- Add the remaining `warden-gate-v1` `aad`/`pad` byte-level vectors to #184 if needed.
