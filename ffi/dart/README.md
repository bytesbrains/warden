# warden_ffi

Dart FFI bindings for **Warden**'s native threshold **conditional-decryption gate** — seal a
blob behind an on-chain condition, and open it once the federation releases the key.

> ⚠️ **EXPERIMENTAL — pre-release.** Warden is a Phase-0 PoC. On an all-ours test federation the
> *timing* guarantee is zero-security (do not claim "unreadable until the condition triggers");
> this gate provides condition-binding only. Treat as preview until a production federation +
> audit. `^` constraints won't auto-select a pre-release — pin it explicitly.

## What it does

A thin, safe wrapper over the `warden_*` C ABI (JSON/hex in, `{ok,value|error}` JSON out):

- `conditionIdentity(conditionJson)` — `H(condition)` hex.
- `sealGated(conditionJson, masterPubHex, network, blobHex)` — gate an already-encrypted blob.
- `openGated(envelopeJson, dIdHex)` — undo the gate with the released key.
- `combine(partialsJson, idHex, fedJson)` — combine `t` threshold partials → `d_id`.

The pairing crypto lives once in audited Rust (`warden/ffi`); this package only marshals.

## Bring your own native library

This package is the Dart binding only — it does **not** ship the native binary. Build it from
the [`warden`](https://github.com/bytesbrains/warden) repo's `ffi` crate:

```sh
# Host dylib (desktop / tests):
cargo build -p warden-ffi          # → target/debug/libwarden_ffi.{dylib,so}

# Mobile artifacts (iOS xcframework + Android jniLibs):
ffi/build-mobile.sh all --out /path/to/your-app/mobile
```

Then load it:

```dart
import 'package:warden_ffi/warden_ffi.dart';

// Desktop/tests: explicit path. iOS/macOS: linked into the process (omit path).
// Android: loads libwarden_ffi.so from jniLibs (omit path).
final w = WardenFfi.load(path: 'target/debug/libwarden_ffi.dylib');

final id  = w.conditionIdentity(conditionJson);
final env = w.sealGated(conditionJson, masterPubHex, network, blobHex);
// … later, once the federation releases the key …
final dId  = w.combine(partialsJson, id, fedJson);
final blob = w.openGated(env, dId);
```

Bad input or a wrong-condition key throws `WardenFfiException` (never a crash — the native
boundary catches panics).

## License

MIT.
