//! Drives the C-ABI surface against the committed cross-language fixture — the FFI analog of
//! the WASM Node round-trip. Calls the `extern "C"` functions exactly as Dart will (C strings
//! in, `{ok,value}` JSON out, free the result).

use std::ffi::{c_char, CStr, CString};

use warden_ffi::{
    warden_combine, warden_condition_identity, warden_open_gated, warden_seal_gated,
    warden_string_free,
};

const FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../wasm/test/fixture.json"
));

/// Read + free an FFI result string, unwrap `{ok,value}`, and panic on `{ok:false,error}`.
fn take(p: *mut c_char) -> String {
    let s = unsafe { CStr::from_ptr(p) }.to_str().unwrap().to_string();
    unsafe { warden_string_free(p) };
    let v: serde_json::Value = serde_json::from_str(&s).unwrap();
    assert!(v["ok"].as_bool().unwrap(), "ffi error: {}", v["error"]);
    v["value"].as_str().unwrap().to_string()
}

fn cs(s: &str) -> CString {
    CString::new(s).unwrap()
}

#[test]
fn ffi_matches_the_cross_language_fixture() {
    let fx: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    let cond = cs(&serde_json::to_string(&fx["condition"]).unwrap());
    let id_hex = fx["identity"].as_str().unwrap();
    let mpk = cs(fx["masterPub"].as_str().unwrap());
    let net = cs(fx["network"].as_str().unwrap());
    let blob = cs(fx["blob"].as_str().unwrap());

    // 1. identity == the fixture + the #207 KAT.
    let id = take(unsafe { warden_condition_identity(cond.as_ptr()) });
    assert_eq!(id, id_hex);
    assert_eq!(
        id,
        "47fce3a147fc844978e8301a7aedbf437100eda9f769ac0d559c85d806cdb68e"
    );

    // 2. combine the fixture's partials → the fixture's d_id.
    let partials = cs(&serde_json::to_string(&fx["partials"]).unwrap());
    let fed = cs(&serde_json::to_string(&fx["federation"]).unwrap());
    let d_id = take(unsafe { warden_combine(partials.as_ptr(), cs(&id).as_ptr(), fed.as_ptr()) });
    assert_eq!(d_id, fx["dId"].as_str().unwrap());

    // 3. open the fixture's gated envelope with d_id → the original blob.
    let fx_env = cs(&serde_json::to_string(&fx["gatedEnvelope"]).unwrap());
    let did_cs = cs(&d_id);
    let recovered = take(unsafe { warden_open_gated(fx_env.as_ptr(), did_cs.as_ptr()) });
    assert_eq!(recovered, fx["blob"].as_str().unwrap());

    // 4. round-trip: FFI seals a NEW gate (fresh obk), opens with the same d_id → same blob.
    let new_env = take(unsafe {
        warden_seal_gated(cond.as_ptr(), mpk.as_ptr(), net.as_ptr(), blob.as_ptr())
    });
    let new_env_cs = cs(&new_env);
    let out = take(unsafe { warden_open_gated(new_env_cs.as_ptr(), did_cs.as_ptr()) });
    assert_eq!(out, fx["blob"].as_str().unwrap());
}

#[test]
fn ffi_errors_return_ok_false_not_panic() {
    // Bad JSON / hex → a clean {ok:false,error}, never a crash across the boundary.
    let bad = cs("not a condition");
    let p = unsafe { warden_condition_identity(bad.as_ptr()) };
    let s = unsafe { CStr::from_ptr(p) }.to_str().unwrap().to_string();
    unsafe { warden_string_free(p) };
    let v: serde_json::Value = serde_json::from_str(&s).unwrap();
    assert_eq!(v["ok"], false);
    assert!(v["error"].is_string());
}
