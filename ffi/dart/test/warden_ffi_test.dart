// Host test for the Dart↔warden-ffi bridge: loads the native dylib + the committed
// cross-language fixture and drives WardenFfi. Mirrors the Rust FFI test and the WASM
// Node round-trip — proving the Dart binding interops with Rust byte-for-byte.
//
// Requires the host dylib to be built first (from the repo root):
//   cargo build -p warden-ffi
// Run from this package dir:  dart test
import 'dart:convert';
import 'dart:io';

import 'package:test/test.dart';
import 'package:warden_ffi/warden_ffi.dart';

void main() {
  // CWD = warden/ffi/dart/ ; the workspace target + fixture are two levels up.
  final ext = Platform.isMacOS ? 'dylib' : 'so';
  final dylib = '../../target/debug/libwarden_ffi.$ext';
  final fixtureFile = File('../../wasm/test/fixture.json');

  if (!File(dylib).existsSync()) {
    // Skip cleanly if the native lib hasn't been built (e.g. a Dart-only CI lane).
    test('warden-ffi host bridge (skipped: native lib not built)', () {}, skip: true);
    return;
  }

  late WardenFfi w;
  late Map<String, dynamic> fx;
  setUpAll(() {
    w = WardenFfi.load(path: dylib);
    fx = jsonDecode(fixtureFile.readAsStringSync()) as Map<String, dynamic>;
  });

  test('conditionIdentity matches the cross-language fixture (KAT)', () {
    expect(w.conditionIdentity(jsonEncode(fx['condition'])), fx['identity']);
  });

  test('combine → fixture d_id; open_gated → blob; seal→open round-trips', () {
    final dId = w.combine(
        jsonEncode(fx['partials']), fx['identity'] as String, jsonEncode(fx['federation']));
    expect(dId, fx['dId']);

    // Open the fixture's gated envelope.
    expect(w.openGated(jsonEncode(fx['gatedEnvelope']), dId), fx['blob']);

    // Seal a fresh gate (random obk) → opens with the same d_id → same blob.
    final env = w.sealGated(
      jsonEncode(fx['condition']),
      fx['masterPub'] as String,
      fx['network'] as String,
      fx['blob'] as String,
    );
    expect(w.openGated(env, dId), fx['blob']);
  });

  test('bad input throws WardenFfiException (not a crash)', () {
    expect(() => w.conditionIdentity('not a condition'), throwsA(isA<WardenFfiException>()));
  });
}
