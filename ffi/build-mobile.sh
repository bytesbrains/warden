#!/usr/bin/env bash
# Cross-compile warden-ffi for the mobile app (Veil native bridge, #181 step 5c).
#
#   ./build-mobile.sh ios       → mobile/ios/WardenFfi.xcframework        (device + simulator)
#   ./build-mobile.sh android   → mobile/android/app/src/main/jniLibs/<abi>/libwarden_ffi.so
#   ./build-mobile.sh all       → both (default)
#
# Outputs are git-ignored build artifacts — regenerate after any change to warden-core/ffi
# (and after a redeploy is irrelevant here: the gate crypto has no on-chain addresses baked in).
#
# Prereqs:
#   iOS     — Xcode + `rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios`
#             (x86_64-apple-ios is the Intel-Mac simulator slice; without it the simulator
#              build fails to link on Intel hosts / x86_64 macOS CI runners)
#   Android — Android NDK + `cargo install cargo-ndk --locked` +
#             `rustup target add aarch64-linux-android armv7-linux-androideabi \
#                                x86_64-linux-android i686-linux-android`
#             For reproducible release builds, pin the NDK explicitly (e.g.
#              ANDROID_NDK_HOME=~/Library/Android/sdk/ndk/27.0.12077973) rather than relying
#              on cargo-ndk autodetect — a different NDK yields a different binary.
#
# The Rust toolchain stays pinned at 1.83 via warden/rust-toolchain.toml; this only adds
# cross-compile *targets*, not a channel bump. Builds pass --locked so the shipped crypto is
# the audited Cargo.lock dependency set. Release profile keeps panic=unwind so the FFI
# catch_unwind guard holds (see warden/Cargo.toml) — never build these with panic=abort.
set -euo pipefail

WARDEN_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="$(cd "$WARDEN_DIR/.." && pwd)"
MOBILE_DIR="$REPO_ROOT/mobile"
LIB="libwarden_ffi"
WHAT="${1:-all}"

build_ios() {
  echo "==> iOS: cross-compiling $LIB (device + simulator arm64+x86_64, release, --locked)"
  ( cd "$WARDEN_DIR" \
      && cargo build --locked -p warden-ffi --release --target aarch64-apple-ios \
      && cargo build --locked -p warden-ffi --release --target aarch64-apple-ios-sim \
      && cargo build --locked -p warden-ffi --release --target x86_64-apple-ios )

  # Fuse the two simulator arches into one universal slice so the xcframework links on both
  # Apple-Silicon and Intel hosts (CTO + Gemini: arm64-only sim breaks Intel/x86_64 CI).
  local sim="$WARDEN_DIR/target/ios-sim-universal/release"
  mkdir -p "$sim"
  lipo -create \
    "$WARDEN_DIR/target/aarch64-apple-ios-sim/release/$LIB.a" \
    "$WARDEN_DIR/target/x86_64-apple-ios/release/$LIB.a" \
    -output "$sim/$LIB.a"

  local out="$MOBILE_DIR/ios/WardenFfi.xcframework"
  echo "==> iOS: assembling $out (ios-arm64 device + universal simulator)"
  rm -rf "$out"
  xcodebuild -create-xcframework \
    -library "$WARDEN_DIR/target/aarch64-apple-ios/release/$LIB.a" \
    -library "$sim/$LIB.a" \
    -output "$out"
  echo "==> iOS: done. Wire it once into ios/Runner per warden/ffi/README.md (§ Mobile → iOS)."
}

build_android() {
  echo "==> Android: cross-compiling $LIB (4 ABIs, release, --locked) via cargo-ndk"
  local jni="$MOBILE_DIR/android/app/src/main/jniLibs"
  ( cd "$WARDEN_DIR" \
      && cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -t x86 \
           -o "$jni" build --locked -p warden-ffi --release )
  echo "==> Android: done. Gradle bundles $jni/<abi>/$LIB.so automatically."
}

case "$WHAT" in
  ios)     build_ios ;;
  android) build_android ;;
  all)     build_ios; build_android ;;
  *) echo "usage: $0 [ios|android|all]" >&2; exit 2 ;;
esac
