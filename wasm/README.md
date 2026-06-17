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

## Next

- Node round-trip test asserting `condition_identity` matches the Rust KAT
  (`47fce3a1…06cdb68e` for the Veil condition) + a `seal_gated`→`combine`→`open_gated` loop
  against a dealt federation.
- Add `warden-gate-v1` `aad`/`pad` cross-language KATs (#184) before the SDK depends on this.
- SDK `veilSeal`/`veilOpen` consume `pkg/`; the app wires the opt-in (preview) Beat flow.
