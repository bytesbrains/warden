//! Shared helpers: load the public federation file, write a secret file (0600 on unix).

use std::fs;

use warden_core::fed::FederationPublic;

/// Load + parse `federation.json`.
pub fn load_federation(path: &str) -> Result<FederationPublic, String> {
    let raw = fs::read_to_string(path).map_err(|e| format!("reading {path}: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("parsing {path}: {e}"))
}

/// Write secret material with owner-only permissions where the platform supports it.
#[cfg(unix)]
pub fn write_secret(path: &str, contents: &str) -> Result<(), String> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| format!("opening {path}: {e}"))?;
    // `mode()` only applies when the file is *created*; tighten an existing file too, so a
    // pre-existing world-readable key file can't keep leaking the secret.
    if let Ok(meta) = f.metadata() {
        let mut perms = meta.permissions();
        if perms.mode() & 0o777 != 0o600 {
            perms.set_mode(0o600);
            let _ = f.set_permissions(perms);
        }
    }
    f.write_all(contents.as_bytes())
        .map_err(|e| format!("writing {path}: {e}"))
}

#[cfg(not(unix))]
pub fn write_secret(path: &str, contents: &str) -> Result<(), String> {
    fs::write(path, contents).map_err(|e| format!("writing {path}: {e}"))
}
