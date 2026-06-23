// Minimal walk-through of the warden_ffi binding.
//
// Prerequisite — build the native library first (from the warden repo root):
//   cargo build -p warden-ffi
//
// Run:  dart run example/warden_ffi_example.dart [path/to/libwarden_ffi.dylib]
//
// This shows how to load the library and the shape of the API. A full
// seal → combine → open round-trip needs a real federation's master public key
// and released partials; see the package's test (`test/warden_ffi_test.dart`),
// which drives the complete flow against the committed cross-language fixture.

import 'dart:io';

import 'package:warden_ffi/warden_ffi.dart';

void main(List<String> args) {
  // Desktop/tests: pass the dylib path. iOS/macOS: omit it (linked into the
  // process). Android: omit it (loaded from jniLibs).
  final libPath = args.isNotEmpty ? args.first : null;
  if (libPath != null && !File(libPath).existsSync()) {
    stderr.writeln('native lib not found at "$libPath" — build it with: '
        'cargo build -p warden-ffi');
    exitCode = 1;
    return;
  }

  final warden = WardenFfi.load(path: libPath);

  // A condition is a JSON predicate over on-chain state: call `fn(args)` on a
  // contract and test the result. The identity is its hash — what a payload is
  // sealed to. (Example only; swap in your own contract/function.)
  const conditionJson = '{'
      '"type":"contract",'
      '"chain":1,'
      '"address":"0x0000000000000000000000000000000000000000",'
      '"fn":"isUnlocked(uint256)",'
      '"args":["42"],'
      '"word":0,'
      '"test":{"cmp":"==","value":true},'
      '"meta":{"finality":32,"tier":1}'
      '}';

  try {
    final identity = warden.conditionIdentity(conditionJson);
    print('condition identity (H(condition)): $identity');
    print('seal with:   warden.sealGated(conditionJson, masterPubHex, network, blobHex)');
    print('combine with: warden.combine(partialsJson, identity, federationJson)');
    print('open with:   warden.openGated(envelopeJson, dId)');
  } on WardenFfiException catch (e) {
    stderr.writeln('warden error: ${e.message}');
    exitCode = 1;
  }
}
