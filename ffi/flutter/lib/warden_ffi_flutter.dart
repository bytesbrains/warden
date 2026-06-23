/// Flutter plugin that bundles the Warden native FFI library (the Veil bridge), so a
/// Flutter app gets the threshold conditional-decryption gate with **no Rust toolchain
/// and no native build wiring**.
///
/// It re-exports the pure [`warden_ffi`](https://pub.dev/packages/warden_ffi) binding;
/// this package's only job is to deliver the matching native library per platform:
///
/// - **iOS** — an embedded dynamic `WardenFfi.framework` (the podspec downloads it from
///   the warden release and CocoaPods embeds + signs it). dyld loads it at launch, so
///   [WardenFfi.load] resolves the symbols via `DynamicLibrary.process()` — no path,
///   no `-force_load`.
/// - **Android** — `jniLibs/<abi>/libwarden_ffi.so` (gradle downloads it from the same
///   release). [WardenFfi.load] resolves it via `DynamicLibrary.open('libwarden_ffi.so')`.
///
/// ```dart
/// import 'package:warden_ffi_flutter/warden_ffi_flutter.dart';
///
/// final warden = WardenFfi.load(); // bundled native lib — no path needed on device
/// final id = warden.conditionIdentity(conditionJson);
/// ```
///
/// ⚠️ Preview: on a single-operator test federation the *timing* guarantee is not
/// security — this gate gives condition-binding only.
library;

export 'package:warden_ffi/warden_ffi.dart';
