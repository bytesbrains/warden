# warden_ffi_flutter

Flutter plugin that **bundles the Warden native FFI library** (the Veil bridge) so a
Flutter app gets the threshold conditional-decryption gate with **no Rust toolchain and
no native build wiring**. It wraps and re-exports the pure
[`warden_ffi`](https://pub.dev/packages/warden_ffi) binding; its only job is to deliver
the matching native library per platform.

> ⚠️ Preview / Phase-0 PoC. On a single-operator test federation the *timing* guarantee
> is not security — this gate gives condition-binding only. Not audited.

## Use

```yaml
dependencies:
  warden_ffi_flutter: 0.1.0-dev.1
```

```dart
import 'package:warden_ffi_flutter/warden_ffi_flutter.dart';

// The native library is bundled by this plugin, so no path is needed on device.
final warden = WardenFfi.load();

final id  = warden.conditionIdentity(conditionJson);
final env = warden.sealGated(conditionJson, masterPubHex, network, blobHex);
// …once the federation releases enough partials for this condition…
final dId  = warden.combine(partialsJson, id, fedJson);
final blob = warden.openGated(env, dId); // == the original blobHex
```

## How the native library is delivered

Binaries are large and platform-specific, so they are **not** shipped inside this package.
They're built once per version by warden CI and published as GitHub Release assets; this
plugin downloads the matching set at build time:

- **iOS** — the [podspec](ios/warden_ffi_flutter.podspec)'s `prepare_command` downloads the
  prebuilt **dynamic** `WardenFfi.xcframework` and CocoaPods embeds + signs it. dyld loads
  it at app launch, so `WardenFfi.load()` resolves the symbols via
  `DynamicLibrary.process()` — no `-force_load`, no Xcode wiring.
- **Android** — [`build.gradle`](android/build.gradle) downloads `jniLibs/<abi>/libwarden_ffi.so`
  and adds it as a `jniLibs` source set; Gradle bundles it into the APK and
  `WardenFfi.load()` resolves it via `DynamicLibrary.open('libwarden_ffi.so')`.

The warden release tag the binaries come from is pinned in both files (`warden_tag` /
`WARDEN_NATIVE_TAG`) and kept in lockstep with this package's version.

## Desktop / tests

For host (desktop/CI) use, depend on the pure `warden_ffi` package directly and pass a
dylib path to `WardenFfi.load(path: …)` — see its
[README](https://pub.dev/packages/warden_ffi). This plugin targets iOS + Android.

## License

MIT — see [LICENSE](LICENSE).
