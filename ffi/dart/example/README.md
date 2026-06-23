# warden_ffi example

A minimal walk-through of loading the binding and the seal → combine → open API.

```sh
# 1. Build the native library (from the warden repo root):
cargo build -p warden-ffi

# 2. Run the example, pointing at the built dylib:
dart run example/warden_ffi_example.dart ../../target/debug/libwarden_ffi.dylib
```

A full round-trip needs a real federation's master public key and released
partials — see `test/warden_ffi_test.dart`, which drives the complete flow
against the committed cross-language fixture.
