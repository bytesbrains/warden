## 0.1.0-dev.1

- Initial pre-release. Flutter plugin that bundles the Warden native FFI library
  (the Veil bridge) and re-exports the `warden_ffi` binding.
- iOS: downloads + embeds the prebuilt dynamic `WardenFfi.xcframework` from the
  warden GitHub release (`v0.1.0-dev.2`) via the podspec.
- Android: downloads the prebuilt `jniLibs/<abi>/libwarden_ffi.so` from the same
  release via `build.gradle`.
- Preview / Phase-0 PoC: the timing guarantee is not security on the test federation.
