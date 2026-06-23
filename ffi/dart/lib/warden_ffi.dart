// Dart FFI bindings for `warden-ffi` — the native threshold conditional-decryption gate.
//
// Mirrors the WASM binding exactly: JSON/hex strings in, `{ok,value|error}` JSON out.
// The pairing crypto lives once in audited Rust (`warden/ffi`); this only marshals. Every
// returned C string is freed via `warden_string_free` (see `_take`). Hex is 0x-less.
//
// ⚠️ PREVIEW. On an all-ours testnet the *timing* guarantee is zero-security — do not claim
// "unreadable until the condition triggers". This gate provides condition-binding only;
// recipient confidentiality (if any) comes from the consuming app's own encryption layer.
import 'dart:convert';
import 'dart:ffi';
import 'dart:io';

import 'package:ffi/ffi.dart';

typedef _StrFnC = Pointer<Utf8> Function(Pointer<Utf8>);
typedef _StrFn = Pointer<Utf8> Function(Pointer<Utf8>);
typedef _Str2FnC = Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>);
typedef _Str2Fn = Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>);
typedef _Str3FnC = Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>, Pointer<Utf8>);
typedef _Str3Fn = Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>, Pointer<Utf8>);
typedef _Str4FnC =
    Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>, Pointer<Utf8>, Pointer<Utf8>);
typedef _Str4Fn =
    Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>, Pointer<Utf8>, Pointer<Utf8>);
typedef _FreeC = Void Function(Pointer<Utf8>);
typedef _Free = void Function(Pointer<Utf8>);

/// A `{ok:false,error}` from the native layer (bad input, wrong-condition key, etc.).
class WardenFfiException implements Exception {
  final String message;
  const WardenFfiException(this.message);
  @override
  String toString() => 'WardenFfiException: $message';
}

/// Thin, safe wrapper over the `warden_*` C ABI. Stateless + reentrant.
///
/// Bring your own native library — build it from the `warden/ffi` crate
/// (`cargo build -p warden-ffi` for a host dylib, or `ffi/build-mobile.sh` for
/// iOS/Android artifacts) and load it via [WardenFfi.load].
class WardenFfi {
  final _StrFn _identity;
  final _Str4Fn _seal;
  final _Str2Fn _open;
  final _Str3Fn _combine;
  final _Free _free;

  WardenFfi._(DynamicLibrary lib)
      : _identity = lib.lookupFunction<_StrFnC, _StrFn>('warden_condition_identity'),
        _seal = lib.lookupFunction<_Str4FnC, _Str4Fn>('warden_seal_gated'),
        _open = lib.lookupFunction<_Str2FnC, _Str2Fn>('warden_open_gated'),
        _combine = lib.lookupFunction<_Str3FnC, _Str3Fn>('warden_combine'),
        _free = lib.lookupFunction<_FreeC, _Free>('warden_string_free');

  /// Load the native lib. iOS/macOS link the static lib into the process; other
  /// platforms load `libwarden_ffi.so`; desktop/tests pass an explicit dylib [path].
  factory WardenFfi.load({String? path}) {
    final DynamicLibrary lib;
    if (path != null) {
      lib = DynamicLibrary.open(path);
    } else if (Platform.isIOS || Platform.isMacOS) {
      lib = DynamicLibrary.process();
    } else {
      lib = DynamicLibrary.open('libwarden_ffi.so');
    }
    return WardenFfi._(lib);
  }

  /// `H(condition)` hex.
  String conditionIdentity(String conditionJson) => _call([conditionJson], (p) => _identity(p[0]));

  /// Gate an already-encrypted `blobHex` on `conditionJson` under `masterPubHex` → envelope JSON.
  String sealGated(String conditionJson, String masterPubHex, String network, String blobHex) =>
      _call([conditionJson, masterPubHex, network, blobHex], (p) => _seal(p[0], p[1], p[2], p[3]));

  /// Undo the gate with the released key → blob hex.
  String openGated(String envelopeJson, String dIdHex) =>
      _call([envelopeJson, dIdHex], (p) => _open(p[0], p[1]));

  /// Verify + dedup + combine `t` partials → `d_id` hex (tolerant of a noisy set).
  String combine(String partialsJson, String idHex, String fedJson) =>
      _call([partialsJson, idHex, fedJson], (p) => _combine(p[0], p[1], p[2]));

  /// Marshal Dart strings → C, invoke, parse `{ok,value}`, and free everything.
  String _call(List<String> args, Pointer<Utf8> Function(List<Pointer<Utf8>>) invoke) {
    final ptrs = args.map((a) => a.toNativeUtf8()).toList();
    try {
      return _take(invoke(ptrs));
    } finally {
      for (final p in ptrs) {
        malloc.free(p);
      }
    }
  }

  String _take(Pointer<Utf8> result) {
    final s = result.toDartString();
    _free(result);
    final m = jsonDecode(s) as Map<String, dynamic>;
    if (m['ok'] != true) {
      throw WardenFfiException(m['error']?.toString() ?? 'unknown native error');
    }
    return m['value'] as String;
  }
}
