#
# warden_ffi_flutter — iOS native delivery for the Warden Veil bridge.
#
# The native library is NOT vendored in the published package (binaries are large and
# platform-specific). Instead `prepare_command` downloads the prebuilt **dynamic**
# WardenFfi.xcframework from the matching warden GitHub release at `pod install` time,
# and CocoaPods embeds + signs it. Because it's a dynamic framework, dyld loads it at
# app launch and the warden_* C symbols resolve via DynamicLibrary.process() with no
# -force_load wiring on the consumer.
#
Pod::Spec.new do |s|
  s.name             = 'warden_ffi_flutter'
  s.version          = '0.1.0-dev.1'
  s.summary          = 'Bundles the Warden native FFI library (Veil bridge) for Flutter iOS.'
  s.homepage         = 'https://github.com/bytesbrains/warden'
  s.license          = { :type => 'MIT', :file => '../LICENSE' }
  s.author           = { 'bytesbrains' => 'contact@bytesbrains.com' }
  s.source           = { :path => '.' }
  s.dependency 'Flutter'
  s.platform = :ios, '12.0'
  s.pod_target_xcconfig = { 'DEFINES_MODULE' => 'YES' }

  # The warden release whose binaries this version pulls. Keep in lockstep with the
  # tag the android/build.gradle downloads (WARDEN_NATIVE_TAG).
  warden_tag = 'v0.1.0-dev.2'
  s.prepare_command = <<-CMD
    set -euo pipefail
    if [ ! -d "WardenFfi.xcframework" ]; then
      url="https://github.com/bytesbrains/warden/releases/download/#{warden_tag}/WardenFfi.xcframework.zip"
      echo "warden_ffi_flutter: downloading $url"
      curl -fsSL "$url" -o WardenFfi.xcframework.zip
      unzip -q -o WardenFfi.xcframework.zip
      rm -f WardenFfi.xcframework.zip
    fi
  CMD

  s.vendored_frameworks = 'WardenFfi.xcframework'
end
