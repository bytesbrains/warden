# warden_ffi

Dart FFI bindings for **Warden** — a threshold **conditional-decryption** gate.

Seal an (already-encrypted) blob so it can only be opened once a federation of independent
nodes agrees that an **on-chain condition has become true** — then open it with the released
key. The cryptography lives once in audited Rust ([`warden`](https://github.com/bytesbrains/warden));
this package is the thin, safe Dart binding over its C ABI.

> ⚠️ **Experimental — pre-release.** Warden is a Phase-0 proof of concept. On a single-operator
> test federation the *timing* guarantee is **not security** — do not claim "unreadable until the
> condition triggers." This gate gives you *condition-binding*; recipient confidentiality (if you
> need it) must come from your own encryption layer underneath. Treat as preview until there's a
> production federation and an audit. Being a pre-release, `^` constraints won't auto-select it —
> depend on it explicitly.

## What problem it solves

You have a payload that should become decryptable **only when some condition is met** — a deadline
passes, a contract flips a flag, an event fires. Warden's federation holds shares of a master key
and releases a per-item decryption key **only** when the condition it was sealed against is
observed on-chain. No single node can release early; no central server can be compelled to.

This package lets a Dart/Flutter program drive that flow natively.

## API

`WardenFfi` is a stateless, reentrant wrapper. JSON/hex in, JSON-validated values out:

| Method | Purpose |
|---|---|
| `conditionIdentity(conditionJson)` | `H(condition)` — the identity a payload is sealed to. |
| `sealGated(conditionJson, masterPubHex, network, blobHex)` | Gate an already-encrypted blob → envelope JSON. |
| `combine(partialsJson, idHex, fedJson)` | Combine `t` threshold partials from the federation → `d_id`. |
| `openGated(envelopeJson, dIdHex)` | Open the gate with the released `d_id` → the original blob. |

Bad input or a wrong-condition key throws `WardenFfiException` — never a crash (the native
boundary catches panics and returns a structured error).

## Bring your own native library

This package is the **binding only** — it does not ship a native binary (they're large and
platform-specific). Build it from the [`warden`](https://github.com/bytesbrains/warden) repo:

```sh
# Desktop / tests — a host dylib:
cargo build -p warden-ffi          # → target/debug/libwarden_ffi.{dylib,so}

# Mobile — iOS xcframework + Android jniLibs, written into your app tree:
ffi/build-mobile.sh all --out /path/to/your-app/mobile
```

Then load it:

```dart
import 'package:warden_ffi/warden_ffi.dart';

// Desktop / tests: pass an explicit path.
// iOS / macOS: linked into the process — call `WardenFfi.load()` (no path).
// Android: loads `libwarden_ffi.so` from jniLibs — `WardenFfi.load()` (no path).
final warden = WardenFfi.load(path: 'target/debug/libwarden_ffi.dylib');

final id  = warden.conditionIdentity(conditionJson);
final env = warden.sealGated(conditionJson, masterPubHex, network, blobHex);

// …later, once the federation has released enough partials for this condition…
final dId  = warden.combine(partialsJson, id, fedJson);
final blob = warden.openGated(env, dId);   // == the original blobHex
```

See [`example/`](example/) for a runnable walk-through, and the Warden repo for the condition
format, federation setup, and the cross-language test fixtures.

## Platform support

| Platform | Loading |
|---|---|
| iOS / macOS | static lib linked into the process (`DynamicLibrary.process()`) |
| Android | `libwarden_ffi.so` from `jniLibs` |
| Linux / Windows / desktop & tests | explicit dylib/.so/.dll path |

Native-only (uses `dart:ffi`) — not supported on the web.

## License

[MIT](LICENSE).
