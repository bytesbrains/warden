//! Federation file format — the on-disk artifacts a trusted-dealer ceremony (or, later, a
//! real DKG) emits, and that nodes and clients load.
//!
//! Two shapes:
//! - [`FederationPublic`] — the **public** file (`federation.json`). The master public key
//!   (clients encrypt against it) plus every node's share public key (clients verify
//!   partials against them). Safe to publish.
//! - [`NodeShareFile`] — a **secret** per-node file (`shares/node-<i>.json`). One node's
//!   Shamir share of the master secret; loaded by `wardend`. Must be kept confidential.
//!
//! `arkworks` types (the master public key, share public keys, the share scalar) are
//! carried as **hex of their compressed canonical serialization** — language- and
//! endianness-stable, and the same bytes the wire protocol uses.
//!
//! Loaders are **defensive**: they validate `1 <= t <= n` (and, for the public file,
//! `share_pubkeys.len() == n`) and cross-check that each decoded index matches the index
//! the file declares — so a corrupt, hand-edited, or swapped file fails closed rather than
//! producing wrong-but-plausible crypto.
//!
//! This module is feature-independent: it references only public types
//! ([`MasterPublicKey`](crate::ibe::MasterPublicKey),
//! [`SharePublicKey`](crate::ibe::SharePublicKey), [`Share`](crate::shamir::Share)) so a
//! production build (`--no-default-features`, no `trusted-dealer`) can still *read*
//! federation files even though it cannot *create* them.

mod node;
mod public;

pub use node::NodeShareFile;
pub use public::{FederationPublic, SharePub};

use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use zeroize::Zeroize;

use crate::shamir::ShareIndex;

/// Errors decoding / validating a federation file.
#[derive(thiserror::Error, Debug)]
pub enum FedError {
    #[error("hex decode: {0}")]
    Hex(#[from] hex::FromHexError),
    #[error("canonical deserialize (corrupt or wrong-curve material)")]
    Deserialize,
    #[error("index mismatch: file declares {file}, material decodes to {decoded}")]
    IndexMismatch {
        file: ShareIndex,
        decoded: ShareIndex,
    },
    #[error("invalid federation parameters: require 1 <= t <= n and share_pubkeys.len() == n (t={t}, n={n})")]
    InvalidParameters { t: usize, n: usize },
}

/// Hex of an arkworks value's compressed canonical form.
///
/// The temporary byte buffer is scrubbed afterwards: for a secret [`Share`](crate::shamir::Share)
/// it briefly holds the share scalar. (The returned hex `String` is *not* scrubbed — it is
/// what gets written to disk; true end-to-end secret zeroization is tracked as before-mainnet
/// work in `core/README.md`.)
pub(crate) fn to_hex<T: CanonicalSerialize>(v: &T) -> String {
    let mut buf = Vec::new();
    v.serialize_compressed(&mut buf)
        .expect("canonical serialization to a Vec is infallible");
    let s = hex::encode(&buf);
    buf.zeroize();
    s
}

/// Decode an arkworks value from hex of its compressed canonical form.
///
/// `deserialize_compressed` validates subgroup membership (`Validate::Yes`), so malformed or
/// off-curve material is rejected here rather than silently combining wrong. The decoded
/// byte buffer is scrubbed afterwards (may hold a secret share scalar).
pub(crate) fn from_hex<T: CanonicalDeserialize>(s: &str) -> Result<T, FedError> {
    let mut bytes = hex::decode(s)?;
    let res = T::deserialize_compressed(bytes.as_slice()).map_err(|_| FedError::Deserialize);
    bytes.zeroize();
    res
}
