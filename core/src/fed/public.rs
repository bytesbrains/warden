//! The public federation file (`federation.json`) — master public key + share public keys.

use serde::{Deserialize, Serialize};

use super::{from_hex, to_hex, FedError};
use crate::ibe::{MasterPublicKey, SharePublicKey};
use crate::shamir::ShareIndex;

/// A node's published share public key (`index` + hex of the G2 point).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SharePub {
    pub index: ShareIndex,
    pub pk: String,
}

/// The public federation file. Safe to distribute to clients.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct FederationPublic {
    /// Federation / master-key label, bound into the envelope AEAD (`Envelope::network`).
    pub network: String,
    /// Threshold: partials from `t` distinct nodes reconstruct the IBE key.
    pub t: usize,
    /// Federation size.
    pub n: usize,
    /// Hex of the canonical-serialized [`MasterPublicKey`].
    pub mpk: String,
    /// Each node's share public key.
    pub share_pubkeys: Vec<SharePub>,
}

impl FederationPublic {
    /// Build from a dealer/DKG output.
    pub fn new(
        network: &str,
        t: usize,
        n: usize,
        mpk: &MasterPublicKey,
        spks: &[SharePublicKey],
    ) -> Self {
        FederationPublic {
            network: network.to_string(),
            t,
            n,
            mpk: to_hex(mpk),
            share_pubkeys: spks
                .iter()
                .map(|s| SharePub {
                    index: s.index,
                    pk: to_hex(s),
                })
                .collect(),
        }
    }

    /// Check the declared parameters are internally consistent: `1 <= t <= n` and exactly
    /// `n` share public keys. Catches a truncated / hand-edited federation file before any
    /// crypto runs.
    pub fn validate(&self) -> Result<(), FedError> {
        if self.t < 1 || self.t > self.n || self.share_pubkeys.len() != self.n {
            return Err(FedError::InvalidParameters {
                t: self.t,
                n: self.n,
            });
        }
        Ok(())
    }

    /// Decode the master public key.
    pub fn master_public_key(&self) -> Result<MasterPublicKey, FedError> {
        from_hex(&self.mpk)
    }

    /// Decode every share public key (for client-side partial verification + combine).
    ///
    /// Validates the federation parameters first, then cross-checks that each decoded point
    /// carries the same index the file declares (so a reordered / tampered entry fails closed).
    pub fn share_public_keys(&self) -> Result<Vec<SharePublicKey>, FedError> {
        self.validate()?;
        self.share_pubkeys
            .iter()
            .map(|s| {
                let pk: SharePublicKey = from_hex(&s.pk)?;
                if pk.index != s.index {
                    return Err(FedError::IndexMismatch {
                        file: s.index,
                        decoded: pk.index,
                    });
                }
                Ok(pk)
            })
            .collect()
    }
}

#[cfg(all(test, feature = "trusted-dealer"))]
mod tests {
    use super::*;
    use crate::dealer::deal;
    use ark_std::rand::{rngs::StdRng, SeedableRng};

    fn sample() -> FederationPublic {
        let mut rng = StdRng::seed_from_u64(11);
        let out = deal(3, 5, &mut rng).unwrap();
        FederationPublic::new("warden-test", out.t, out.n, &out.mpk, &out.share_pubkeys)
    }

    #[test]
    fn round_trips_through_json_and_decodes() {
        let mut rng = StdRng::seed_from_u64(11);
        let out = deal(3, 5, &mut rng).unwrap();
        let pubf = FederationPublic::new("warden-test", out.t, out.n, &out.mpk, &out.share_pubkeys);

        let json = serde_json::to_string(&pubf).unwrap();
        let back: FederationPublic = serde_json::from_str(&json).unwrap();
        assert_eq!(pubf, back);

        assert_eq!(back.master_public_key().unwrap(), out.mpk);
        assert_eq!(back.share_public_keys().unwrap(), out.share_pubkeys);
    }

    #[test]
    fn corrupt_mpk_hex_is_rejected() {
        let mut pubf = sample();
        pubf.mpk = "00".repeat(48); // valid hex, not a valid compressed G2 point
        assert!(pubf.master_public_key().is_err());
    }

    #[test]
    fn bad_parameters_are_rejected() {
        let mut pubf = sample();
        pubf.t = 9; // t > n
        assert!(matches!(
            pubf.share_public_keys(),
            Err(FedError::InvalidParameters { t: 9, n: 5 })
        ));

        let mut short = sample();
        short.share_pubkeys.pop(); // len != n
        assert!(matches!(
            short.share_public_keys(),
            Err(FedError::InvalidParameters { .. })
        ));
    }

    #[test]
    fn share_pubkey_index_tamper_is_caught() {
        let mut pubf = sample();
        pubf.share_pubkeys[0].index = 99; // declared index no longer matches the point
        assert!(matches!(
            pubf.share_public_keys(),
            Err(FedError::IndexMismatch {
                file: 99,
                decoded: 1
            })
        ));
    }
}
