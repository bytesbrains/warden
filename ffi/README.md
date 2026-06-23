# warden-ffi

A thin **C-ABI** over `warden-core` for a Flutter consuming app via `dart:ffi` (e.g. Maktub's Veil
layer). Mirrors the WASM/SDK boundary exactly, so the pairing crypto lives once in Rust and neither
a WASM nor an FFI consumer reimplements it.

> ⚠️ Not audited. PoC. The *timing* guarantee is zero-security on the all-ours testnet — ship "preview",
> never claim unreadable-until-trigger. Recipient confidentiality stays in the consuming app's hybrid layer.

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

**No secret-bearing inputs** — no master secret, no recipient private key (the consuming app keeps
recipient confidentiality in its own hybrid layer; `open_gated` returns the still-host-encrypted blob).

## Build

`crate-type = ["cdylib", "staticlib", "rlib"]` — `cdylib` → Android `.so` / desktop dylib;
`staticlib` → iOS `.a`; `rlib` → the in-crate host tests.

```bash
cargo test -p warden-ffi      # host round-trip vs the committed fixture (warden/wasm/test/fixture.json)
cargo build -p warden-ffi     # host dylib for `flutter test` (mobile/test/.../warden_ffi_test.dart)
```

The release profile keeps `panic = "unwind"` (see `warden/Cargo.toml`): the boundary's
`catch_unwind` guard only works while panics unwind. **Never build the mobile libs with
`panic = "abort"`** — a panic would kill the host app instead of returning `{"ok":false,…}`.

### Mobile (cross-compile)

One script builds both platforms. Artifacts default to `dist/mobile` inside this repo (git-ignored
— regenerate from source; never commit a prebuilt binary). Pass `--out <dir>` to write into a
consuming app's mobile tree instead:

```bash
ffi/build-mobile.sh ios        # → dist/mobile/ios/WardenFfi.xcframework  (device + simulator)
ffi/build-mobile.sh android    # → dist/mobile/android/app/src/main/jniLibs/<abi>/libwarden_ffi.so
ffi/build-mobile.sh all        # both (default)
ffi/build-mobile.sh all --out /path/to/your-app/mobile   # write straight into a consumer tree
```

Prereqs: iOS — Xcode + `rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios`
(the `x86_64-apple-ios` slice is fused into the simulator library with `lipo` so the xcframework
links on Intel Macs / x86_64 macOS CI too). Android — the NDK + `cargo install cargo-ndk --locked`
+ `rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
i686-linux-android`; pin the NDK explicitly for reproducible release builds (e.g.
`ANDROID_NDK_HOME=~/Library/Android/sdk/ndk/<version>`). Builds run `--locked` so the shipped
crypto is the audited `Cargo.lock` set. Adding *targets* does not bump the pinned 1.83 channel.

**Android** is turn-key: Gradle bundles anything under `jniLibs/<abi>/` and `DynamicLibrary.open('libwarden_ffi.so')` resolves at runtime. Nothing else to wire.

**iOS** needs a one-time Xcode wiring of the xcframework into the `Runner` target (operator step;
not scriptable here without a device build):

1. In Xcode, drag `mobile/ios/WardenFfi.xcframework` into the **Runner** target → *Frameworks,
   Libraries, and Embedded Content* (it is a static lib — "Do Not Embed").
2. The app references the `warden_*` symbols only at runtime via `DynamicLibrary.process()`, so
   a static archive contributes nothing at link time and the symbols are never pulled in — they
   must be force-loaded. Set **SDK-conditional** *Other Linker Flags* (`OTHER_LDFLAGS`) on the
   **Runner** target so each SDK links its matching slice (the xcframework is referenced from
   `$(SRCROOT)`, not copied into `$(BUILT_PRODUCTS_DIR)`):
   ```
   OTHER_LDFLAGS[sdk=iphoneos*]        = -force_load "$(SRCROOT)/WardenFfi.xcframework/ios-arm64/libwarden_ffi.a"
   OTHER_LDFLAGS[sdk=iphonesimulator*] = -force_load "$(SRCROOT)/WardenFfi.xcframework/ios-arm64_x86_64-simulator/libwarden_ffi.a"
   ```
   (Avoid `-ObjC -all_load` here — it force-loads *every* static lib in the Flutter pod graph,
   risking duplicate-symbol errors. The targeted `-force_load` above only pulls warden-ffi.)
3. Build onto a device with the explicit-build flow (never bare `flutter install` — see
   `mobile/CLAUDE.md`): `flutter build ios` then install.

Symbol export is verified in both artifacts (`nm`): all five `warden_*` functions are global
text symbols (iOS carries the leading-underscore C-ABI form; `process()` resolves the bare name).

### Status

A consuming app's Dart bridge and its host test pass byte-for-byte against the same fixture as the
Rust + WASM paths (Maktub's reference bridge lives at `mobile/lib/services/crypto/veil/warden_ffi.dart`).
Cross-compilation to iOS (xcframework) and Android (4-ABI jniLibs) is wired and verified.
**Remaining (consumer-side):** wire the gate into the consuming app's create/open flow (gate-over-hybrid:
the app produces its own hybrid envelope, the FFI adds the condition gate, the federation poll releases it).
