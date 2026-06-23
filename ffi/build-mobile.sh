#!/usr/bin/env bash
# Cross-compile warden-ffi for the mobile app (Veil native bridge, #181 step 5c).
#
#   ./build-mobile.sh ios       → <out>/ios/WardenFfi.xcframework        (device + simulator)
#   ./build-mobile.sh android   → <out>/android/app/src/main/jniLibs/<abi>/libwarden_ffi.so
#   ./build-mobile.sh all       → both (default)
#
# Output base (<out>):
#   --out <dir>   write artifacts under <dir> instead of the default. A downstream consumer
#                 (e.g. the Maktub app post-split) points this at its own mobile/ tree:
#                   ./build-mobile.sh all --out /path/to/maktub/mobile
#   default       <warden>/dist/mobile — self-contained, inside this repo, git-ignored.
#                 (Standalone repo: there is no monorepo `$REPO_ROOT/mobile` to reach into.)
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
LIB="libwarden_ffi"

usage() { echo "usage: $0 [ios|ios-framework|android|all] [--out <dir>]" >&2; }

WHAT=""
OUT_DIR=""
while [ $# -gt 0 ]; do
  case "$1" in
    ios|ios-framework|android|all) WHAT="$1"; shift ;;
    --out)           [ $# -ge 2 ] || { echo "--out requires a directory" >&2; usage; exit 2; }
                     OUT_DIR="$2"; shift 2 ;;
    --out=*)         OUT_DIR="${1#*=}"; shift ;;
    -h|--help)       usage; exit 0 ;;
    *)               echo "unknown argument: $1" >&2; usage; exit 2 ;;
  esac
done

WHAT="${WHAT:-all}"
# Default to a self-contained, git-ignored build dir inside this repo — no monorepo path
# assumption. Consumers override with --out to drop artifacts straight into their app tree.
MOBILE_DIR="${OUT_DIR:-$WARDEN_DIR/dist/mobile}"

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
  mkdir -p "$(dirname "$out")"   # the --out base may not pre-exist (e.g. default dist/mobile)
  rm -rf "$out"
  xcodebuild -create-xcframework \
    -library "$WARDEN_DIR/target/aarch64-apple-ios/release/$LIB.a" \
    -library "$sim/$LIB.a" \
    -output "$out"
  echo "==> iOS: done. Wire it once into ios/Runner per warden/ffi/README.md (§ Mobile → iOS)."
}

# Wrap a single dylib slice (device, or universal-simulator) in a .framework bundle.
# The dynamic-framework form is what the warden_ffi_flutter plugin ships: CocoaPods embeds
# + signs it, so dyld loads it at app launch and the warden_* symbols resolve via
# DynamicLibrary.process() with NO -force_load wiring on the consumer.
make_framework() {
  local dylib="$1" fwdir="$2" minos="$3"
  rm -rf "$fwdir"; mkdir -p "$fwdir"
  cp "$dylib" "$fwdir/WardenFfi"
  # dyld locates the binary by its framework-relative @rpath install name.
  install_name_tool -id @rpath/WardenFfi.framework/WardenFfi "$fwdir/WardenFfi"
  cat > "$fwdir/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>WardenFfi</string>
  <key>CFBundleIdentifier</key><string>com.bytesbrains.WardenFfi</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleName</key><string>WardenFfi</string>
  <key>CFBundlePackageType</key><string>FMWK</string>
  <key>CFBundleShortVersionString</key><string>1.0</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>MinimumOSVersion</key><string>$minos</string>
</dict>
</plist>
PLIST
}

build_ios_framework() {
  echo "==> iOS (dynamic): cross-compiling $LIB cdylib (device + simulator arm64+x86_64, release, --locked)"
  ( cd "$WARDEN_DIR" \
      && cargo build --locked -p warden-ffi --release --target aarch64-apple-ios \
      && cargo build --locked -p warden-ffi --release --target aarch64-apple-ios-sim \
      && cargo build --locked -p warden-ffi --release --target x86_64-apple-ios )

  local work="$WARDEN_DIR/target/ios-framework"
  rm -rf "$work"; mkdir -p "$work/device" "$work/sim"

  # Device: the lone arm64 dylib. Xcode reads the Mach-O platform, so create-xcframework
  # files this under the ios-arm64 slice automatically.
  make_framework "$WARDEN_DIR/target/aarch64-apple-ios/release/$LIB.dylib" \
                 "$work/device/WardenFfi.framework" "12.0"
  # Simulator: fuse arm64 + x86_64 so it links on both Apple-Silicon and Intel hosts.
  lipo -create \
    "$WARDEN_DIR/target/aarch64-apple-ios-sim/release/$LIB.dylib" \
    "$WARDEN_DIR/target/x86_64-apple-ios/release/$LIB.dylib" \
    -output "$work/sim-universal.dylib"
  make_framework "$work/sim-universal.dylib" "$work/sim/WardenFfi.framework" "12.0"

  local out="$MOBILE_DIR/ios/WardenFfi.xcframework"
  echo "==> iOS: assembling dynamic $out (ios-arm64 device + universal simulator)"
  mkdir -p "$(dirname "$out")"   # the --out base may not pre-exist (e.g. default dist/mobile)
  rm -rf "$out"
  xcodebuild -create-xcframework \
    -framework "$work/device/WardenFfi.framework" \
    -framework "$work/sim/WardenFfi.framework" \
    -output "$out"
  echo "==> iOS: done (dynamic framework — CocoaPods embeds it; symbols load at launch, no -force_load)."
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
  ios)           build_ios ;;            # static .a + -force_load (legacy / manual wiring)
  ios-framework) build_ios_framework ;;  # dynamic framework (warden_ffi_flutter plugin)
  android)       build_android ;;
  all)           build_ios; build_android ;;
esac
