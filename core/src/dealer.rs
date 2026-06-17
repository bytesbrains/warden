//! Trusted-dealer setup (Phase 0 / testnet only). Gated behind the `trusted-dealer` feature.
//!
//! Generates the master secret centrally and Shamir-splits it into per-node shares, and
//! emits each node's **share public key** (so partials can be verified — see
//! [`crate::ibe::combine_verified`]). The full secret briefly exists in this process —
//! acceptable for a testnet where all nodes are ours and hold no real secrets. **Mainnet
//! replaces this with a real DKG** (same share *shape*, but the master secret is never
//! assembled). See `warden/docs/03-protocol.md`.

use ark_bls12_381::G2Projective;
use ark_ec::Group;
use ark_ff::PrimeField;

use crate::ibe::{MasterKey, MasterPublicKey, SharePublicKey};
use crate::shamir::{split, ShamirError, Share};
use ark_std::rand::{CryptoRng, Rng};

/// Output of a trusted-dealer ceremony. The master secret is intentionally **not** returned
/// (it is dropped/scrubbed inside [`deal`]); only the public key, the per-node shares, and
/// the per-node share public keys survive.
pub struct DealerOutput {
    pub mpk: MasterPublicKey,
    pub shares: Vec<Share>,
    pub share_pubkeys: Vec<SharePublicKey>,
    pub t: usize,
    pub n: usize,
}

/// Deal a `t`-of-`n` federation. `rng` must be a CSPRNG. Errors per [`ShamirError`].
pub fn deal<R: Rng + CryptoRng + ?Sized>(
    t: usize,
    n: usize,
    rng: &mut R,
) -> Result<DealerOutput, ShamirError> {
    let msk = MasterKey::generate(rng);
    let mpk = msk.public();
    let shares = split(msk.secret(), t, n, rng)?;
    let g2 = G2Projective::generator();
    let share_pubkeys = shares
        .iter()
        .map(|s| SharePublicKey {
            index: s.index,
            pk: g2.mul_bigint(s.value.into_bigint()),
        })
        .collect();
    // `msk` drops here (best-effort scrub); only mpk + shares + share_pubkeys survive.
    Ok(DealerOutput {
        mpk,
        shares,
        share_pubkeys,
        t,
        n,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_std::rand::{rngs::StdRng, SeedableRng};

    #[test]
    fn deals_n_shares_and_pubkeys() {
        let mut rng = StdRng::seed_from_u64(7);
        let out = deal(3, 5, &mut rng).unwrap();
        assert_eq!(out.shares.len(), 5);
        assert_eq!(out.share_pubkeys.len(), 5);
        assert_eq!(out.t, 3);
        let idxs: Vec<u64> = out.shares.iter().map(|s| s.index).collect();
        assert_eq!(idxs, vec![1, 2, 3, 4, 5]);
        // Share pubkeys line up with share indices.
        assert!(out
            .shares
            .iter()
            .zip(&out.share_pubkeys)
            .all(|(s, p)| s.index == p.index));
    }

    #[test]
    fn bad_threshold_propagates() {
        let mut rng = StdRng::seed_from_u64(7);
        assert!(deal(9, 5, &mut rng).is_err());
    }
}
