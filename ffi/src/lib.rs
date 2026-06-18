//! Warden C-ABI FFI over `warden-core`, for the Flutter app via `dart:ffi` (Veil).
//!
//! Mirrors the WASM/SDK boundary exactly — **JSON/hex strings in, JSON out** — so the pairing
//! crypto lives once in audited Rust and the app never reimplements it. Each function returns a
//! malloc'd C string of `{"ok":true,"value":...}` or `{"ok":false,"error":...}`; the caller must
//! free it with [`warden_string_free`]. Panics are caught at the boundary (never unwind into Dart).
//!
//! Boundary: hex is **0x-less**; identity / `d_id` / partials / pubkeys are compressed-canonical
//! hex; conditions and the gate envelope are JSON. No secret-bearing inputs (no master secret, no
//! recipient private key — the app keeps recipient confidentiality in its own hybrid layer).
//!
//! ⚠️ Not audited. PoC.

#![forbid(unsafe_op_in_unsafe_fn)]

use std::ffi::{c_char, CStr, CString};
use std::panic::catch_unwind;

use ark_bls12_381::G1Projective;
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use rand::rngs::OsRng;

use warden_core::condition::Condition;
use warden_core::envelope::{self, GatedEnvelope};
use warden_core::fed::FederationPublic;
use warden_core::ibe::{combine_tolerant, MasterPublicKey, Partial};

/// Read a borrowed C string into an owned `String` (copies; safe across the call).
unsafe fn owned(p: *const c_char) -> Result<String, String> {
    if p.is_null() {
        return Err("null pointer".into());
    }
    unsafe { CStr::from_ptr(p) }
        .to_str()
        .map(str::to_string)
        .map_err(|_| "invalid utf-8".into())
}

fn de_hex<T: CanonicalDeserialize>(h: &str, what: &str) -> Result<T, String> {
    let bytes = hex::decode(h).map_err(|e| e.to_string())?;
    T::deserialize_compressed(bytes.as_slice()).map_err(|_| format!("invalid {what}"))
}

fn ser_hex<T: CanonicalSerialize>(v: &T) -> String {
    let mut buf = Vec::new();
    v.serialize_compressed(&mut buf).expect("serialize to Vec");
    hex::encode(buf)
}

/// Wrap a result as a malloc'd `{ok,value|error}` JSON C string (the only allocation the caller frees).
fn ret(r: Result<String, String>) -> *mut c_char {
    let v = match r {
        Ok(value) => serde_json::json!({ "ok": true, "value": value }),
        Err(error) => serde_json::json!({ "ok": false, "error": error }),
    };
    CString::new(v.to_string())
        .unwrap_or_else(|_| CString::new(r#"{"ok":false,"error":"nul in output"}"#).unwrap())
        .into_raw()
}

/// Run `f` at the FFI boundary: catch panics (no unwinding into Dart) and wrap the result.
fn guard(f: impl FnOnce() -> Result<String, String> + std::panic::UnwindSafe) -> *mut c_char {
    ret(catch_unwind(f).unwrap_or_else(|_| Err("panic in warden-ffi".into())))
}

/// Free a string returned by any `warden_*` function.
///
/// # Safety
/// `s` must be a pointer previously returned by this library (or null).
#[no_mangle]
pub unsafe extern "C" fn warden_string_free(s: *mut c_char) {
    if !s.is_null() {
        unsafe { drop(CString::from_raw(s)) };
    }
}

/// `H(condition)` hex. `condition_json` is the JSON condition.
///
/// # Safety
/// `condition_json` must be a valid NUL-terminated UTF-8 C string (or null).
#[no_mangle]
pub unsafe extern "C" fn warden_condition_identity(condition_json: *const c_char) -> *mut c_char {
    let json = unsafe { owned(condition_json) };
    guard(move || {
        let cond: Condition = serde_json::from_str(&json?).map_err(|e| e.to_string())?;
        Ok(hex::encode(cond.identity().map_err(|e| e.to_string())?))
    })
}

/// Gate an already-encrypted `blob_hex` on `condition_json` under `master_pub_hex`. Returns the
/// `warden-gate-v1` envelope JSON.
///
/// # Safety
/// All arguments must be valid NUL-terminated UTF-8 C strings (or null).
#[no_mangle]
pub unsafe extern "C" fn warden_seal_gated(
    condition_json: *const c_char,
    master_pub_hex: *const c_char,
    network: *const c_char,
    blob_hex: *const c_char,
) -> *mut c_char {
    let (cj, mpk, net, blob) = unsafe {
        (
            owned(condition_json),
            owned(master_pub_hex),
            owned(network),
            owned(blob_hex),
        )
    };
    guard(move || {
        let cond: Condition = serde_json::from_str(&cj?).map_err(|e| e.to_string())?;
        let mpk: MasterPublicKey = de_hex(&mpk?, "master public key")?;
        let blob = hex::decode(blob?).map_err(|e| e.to_string())?;
        let env = envelope::seal_gated(cond, &mpk, &net?, &blob, &mut OsRng)
            .map_err(|e| e.to_string())?;
        serde_json::to_string(&env).map_err(|e| e.to_string())
    })
}

/// Open a `warden-gate-v1` envelope (JSON) with the released key `d_id_hex`. Returns the blob hex.
///
/// # Safety
/// Both arguments must be valid NUL-terminated UTF-8 C strings (or null).
#[no_mangle]
pub unsafe extern "C" fn warden_open_gated(
    envelope_json: *const c_char,
    d_id_hex: *const c_char,
) -> *mut c_char {
    let (ej, did) = unsafe { (owned(envelope_json), owned(d_id_hex)) };
    guard(move || {
        let env: GatedEnvelope = serde_json::from_str(&ej?).map_err(|e| e.to_string())?;
        let d_id: G1Projective = de_hex(&did?, "d_id")?;
        Ok(hex::encode(
            envelope::open_gated(&env, &d_id).map_err(|e| e.to_string())?,
        ))
    })
}

/// Verify + Lagrange-combine `t` of the node `partials_json` (a JSON array of hex partials) into
/// `d_id` hex. Tolerant of a noisy set: malformed / wrong-index / invalid / duplicate partials are
/// dropped; errors only if fewer than `t` valid remain.
///
/// # Safety
/// All arguments must be valid NUL-terminated UTF-8 C strings (or null).
#[no_mangle]
pub unsafe extern "C" fn warden_combine(
    partials_json: *const c_char,
    id_hex: *const c_char,
    fed_json: *const c_char,
) -> *mut c_char {
    let (pj, idh, fj) = unsafe { (owned(partials_json), owned(id_hex), owned(fed_json)) };
    guard(move || {
        let hexes: Vec<String> = serde_json::from_str(&pj?).map_err(|e| e.to_string())?;
        let id = hex::decode(idh?).map_err(|e| e.to_string())?;
        let fed: FederationPublic = serde_json::from_str(&fj?).map_err(|e| e.to_string())?;
        let spks = fed.share_public_keys().map_err(|e| e.to_string())?;
        // Parse partials, dropping malformed-hex ones; core does verify + dedup + threshold.
        let partials: Vec<Partial> = hexes
            .iter()
            .filter_map(|h| de_hex::<Partial>(h, "partial").ok())
            .collect();
        let d_id = combine_tolerant(&partials, &id, &spks, fed.t).map_err(|e| e.to_string())?;
        Ok(ser_hex(&d_id))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression guard for the `panic = "unwind"` invariant (`warden/Cargo.toml`). `guard`
    /// must turn a Rust panic into `{"ok":false,...}`, never abort the host app. Under
    /// `panic = "abort"` `catch_unwind` is a no-op and this test aborts the test binary
    /// instead of returning — failing the build, which is exactly the protection the
    /// invariant needs (CISO #214). The `ffi_errors_return_ok_false_not_panic` integration
    /// test only covers the `Err` path; this covers an actual unwinding panic.
    #[test]
    fn guard_catches_panic_returns_ok_false() {
        // Silence the default panic hook so the expected panic doesn't spam test output.
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let p = guard(|| panic!("boom inside guard"));
        std::panic::set_hook(prev);

        assert!(!p.is_null());
        let s = unsafe { CStr::from_ptr(p) }.to_str().unwrap().to_string();
        unsafe { warden_string_free(p) };
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["ok"], false);
        assert_eq!(v["error"], "panic in warden-ffi");
    }
}
