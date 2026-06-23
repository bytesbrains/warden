# Changelog

## 0.1.0-dev.1

- First public **pre-release**. Dart FFI bindings over the `warden_*` C ABI:
  `conditionIdentity` / `sealGated` / `openGated` / `combine`, with
  `WardenFfi.load({path})` and `WardenFfiException` for native errors.
- Bring-your-own native library (build from the `warden/ffi` Rust crate).
- **Experimental:** Warden is a Phase-0 PoC; the timing guarantee is unproven on
  a production federation. Pre-release — pin explicitly.
