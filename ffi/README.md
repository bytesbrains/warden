# warden-ffi

A thin **C-ABI** over `warden-core` for the Flutter app via `dart:ffi` (Veil). Mirrors the
WASM/SDK boundary exactly, so the pairing crypto lives once in audited Rust and neither the SDK
(WASM) nor the app (FFI) reimplements it.

> ⚠️ Not audited. PoC. Veil's *timing* is zero-security on the all-ours testnet — ship "preview",
> never claim unreadable-until-trigger. Recipient confidentiality is real (the app's hybrid layer).

## ABI

Every function takes NUL-terminated UTF-8 C strings and returns a malloc'd C string of
`{"ok":true,"value":…}` or `{"ok":false,"error":…}`. **The caller must free the result with
`warden_string_free`.** Panics are caught at the boundary (never unwind into Dart). Hex is
**0x-less**.

| fn | in → out |
|---|---|
| `warden_condition_identity(conditionJson)` | → `H(condition)` hex |
| `warden_seal_gated(conditionJson, masterPubHex, network, blobHex)` | → `warden-gate-v1` envelope JSON |
| `warden_open_gated(envelopeJson, dIdHex)` | → blob hex |
| `warden_combine(partialsJson, idHex, fedJson)` | → `d_id` hex (verifies + dedups + tolerates a noisy set) |
| `warden_string_free(ptr)` | free a returned string |

**No secret-bearing inputs** — no master secret, no recipient private key (the app keeps recipient
confidentiality in its own hybrid layer; `open_gated` returns the still-host-encrypted blob).

## Build

`crate-type = ["cdylib", "staticlib", "rlib"]` — `cdylib` → Android `.so` / desktop dylib;
`staticlib` → iOS `.a`; `rlib` → the in-crate host tests.

```bash
cargo test -p warden-ffi                                   # host round-trip vs the committed fixture
# iOS (device + sim) static libs:
cargo build -p warden-ffi --release --target aarch64-apple-ios
cargo build -p warden-ffi --release --target aarch64-apple-ios-sim
# Android (NDK targets), e.g.:
cargo build -p warden-ffi --release --target aarch64-linux-android
```

## Next (the Dart side)

- `ffigen` the Dart bindings from a generated C header (or hand-write the ~5 `Pointer<Utf8>`
  signatures), wrap in an ergonomic `Veil` Dart class (free results, parse `{ok,value}`).
- Bundle the native lib (iOS xcframework / Android jniLibs) into `mobile/`.
- Wire the gate into `mobile/lib/services/crypto/` — the app already produces the v2 hybrid
  envelope; the FFI adds the condition gate (`gate-over-hybrid`, same layering as the SDK).
