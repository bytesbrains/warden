//! A per-node secret share file (`shares/node-<i>.json`). **Confidential** — the node's
//! piece of the master secret.

use serde::{Deserialize, Serialize};

use super::{from_hex, to_hex, FedError};
use crate::ibe::MasterPublicKey;
use crate::shamir::{Share, ShareIndex};

/// A per-node secret share file.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct NodeShareFile {
    /// Federation label this share belongs to (must match the public file).
    pub network: String,
    /// Threshold (informational; the client enforces it).
    pub t: usize,
    /// Federation size (informational).
    pub n: usize,
    /// This node's 1-based index.
    pub index: ShareIndex,
    /// Hex of the canonical-serialized [`Share`] (carries its own index + scalar).
    pub share: String,
    /// Hex of the [`MasterPublicKey`] (so a node can sanity-check / serve it).
    pub mpk: String,
}

impl NodeShareFile {
    /// Build from a single Shamir share.
    pub fn new(network: &str, t: usize, n: usize, share: &Share, mpk: &MasterPublicKey) -> Self {
        NodeShareFile {
            network: network.to_string(),
            t,
            n,
            index: share.index,
            share: to_hex(share),
            mpk: to_hex(mpk),
        }
    }

    /// Decode this node's share, validating `1 <= t <= n` and cross-checking that the
    /// embedded share index matches the declared `index` (catches a renamed / swapped file).
    pub fn share(&self) -> Result<Share, FedError> {
        if self.t < 1 || self.t > self.n {
            return Err(FedError::InvalidParameters {
                t: self.t,
                n: self.n,
            });
        }
        let s: Share = from_hex(&self.share)?;
        if s.index != self.index {
            return Err(FedError::IndexMismatch {
                file: self.index,
                decoded: s.index,
            });
        }
        Ok(s)
    }

    /// Decode the master public key.
    pub fn master_public_key(&self) -> Result<MasterPublicKey, FedError> {
        from_hex(&self.mpk)
    }
}

#[cfg(all(test, feature = "trusted-dealer"))]
mod tests {
    use super::*;
    use crate::dealer::deal;
    use ark_std::rand::{rngs::StdRng, SeedableRng};

    #[test]
    fn round_trips_and_share_matches() {
        let mut rng = StdRng::seed_from_u64(12);
        let out = deal(2, 3, &mut rng).unwrap();
        let nf = NodeShareFile::new("warden-test", out.t, out.n, &out.shares[1], &out.mpk);

        let json = serde_json::to_string(&nf).unwrap();
        let back: NodeShareFile = serde_json::from_str(&json).unwrap();
        assert_eq!(nf, back);

        let share = back.share().unwrap();
        assert_eq!(share, out.shares[1]);
        assert_eq!(share.index, 2);
        assert_eq!(back.master_public_key().unwrap(), out.mpk);
    }

    #[test]
    fn detects_index_mismatch() {
        let mut rng = StdRng::seed_from_u64(13);
        let out = deal(2, 3, &mut rng).unwrap();
        let mut nf = NodeShareFile::new("warden-test", out.t, out.n, &out.shares[0], &out.mpk);
        nf.index = 99; // declared index no longer matches the embedded share
        assert!(matches!(
            nf.share(),
            Err(FedError::IndexMismatch {
                file: 99,
                decoded: 1
            })
        ));
    }

    #[test]
    fn rejects_bad_parameters() {
        let mut rng = StdRng::seed_from_u64(14);
        let out = deal(2, 3, &mut rng).unwrap();
        let mut nf = NodeShareFile::new("warden-test", out.t, out.n, &out.shares[0], &out.mpk);
        nf.t = 9; // t > n
        assert!(matches!(
            nf.share(),
            Err(FedError::InvalidParameters { t: 9, n: 3 })
        ));
    }
}
