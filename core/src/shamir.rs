//! Shamir secret sharing over the BLS12-381 scalar field `Fr`, plus Lagrange
//! interpolation at `x = 0`.
//!
//! For Warden this provides:
//! - the **trusted-dealer** split of the master secret into per-node shares (Phase 0;
//!   real DKG replaces this for mainnet — same share *shape*), and
//! - the **Lagrange coefficients** used to combine `t` partial decryptions
//!   (`partial_i = sk_i · H1(identity)`) into the IBE decryption key
//!   `s · H1(identity)` — the combine happens in the group, but the coefficients
//!   are field elements computed here (see [`lagrange_at_zero`]).
//!
//! Shares use x-coordinate = node index `1..=n` (never `0`, which is the secret).

use ark_bls12_381::Fr;
use ark_ff::{Field, One, UniformRand, Zero};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use ark_std::rand::{CryptoRng, Rng};

/// 1-based share / node index. `0` is reserved for the secret.
pub type ShareIndex = u64;

/// Errors from secret sharing.
#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum ShamirError {
    /// `split` requires `1 <= t <= n`.
    #[error("invalid threshold: require 1 <= t <= n (t={t}, n={n})")]
    BadThreshold { t: usize, n: usize },
}

/// A single share `(index, f(index))` of a secret polynomial.
#[derive(Clone, Copy, Debug, PartialEq, Eq, CanonicalSerialize, CanonicalDeserialize)]
pub struct Share {
    pub index: ShareIndex,
    pub value: Fr,
}

/// Split `secret` into `n` shares with threshold `t` (`t` shares reconstruct; fewer learn nothing).
///
/// Builds `f(x) = secret + a_1 x + … + a_{t-1} x^{t-1}` with random `a_i`, and evaluates at
/// `x = 1..=n`. Returns [`ShamirError::BadThreshold`] unless `1 <= t <= n`.
///
/// `rng` must be a cryptographically secure RNG.
pub fn split<R: Rng + CryptoRng + ?Sized>(
    secret: Fr,
    t: usize,
    n: usize,
    rng: &mut R,
) -> Result<Vec<Share>, ShamirError> {
    if t < 1 || t > n {
        return Err(ShamirError::BadThreshold { t, n });
    }
    let mut coeffs = Vec::with_capacity(t);
    coeffs.push(secret);
    for _ in 1..t {
        coeffs.push(Fr::rand(rng));
    }
    let shares = (1..=n as u64)
        .map(|i| {
            let x = Fr::from(i);
            // Horner evaluation of the polynomial at x.
            let mut acc = Fr::zero();
            for c in coeffs.iter().rev() {
                acc = acc * x + c;
            }
            Share {
                index: i,
                value: acc,
            }
        })
        .collect();
    Ok(shares)
}

/// Lagrange basis coefficient `λ_i(0) = Π_{j≠i} (0 - x_j)/(x_i - x_j)` for index `i`
/// over the participating share `indices`.
///
/// `Σ_i λ_i(0) · share_i = f(0) = secret`. The same `λ_i` scale partial signatures in the
/// group during threshold combine. `indices` must be distinct and contain `i` (the caller —
/// e.g. [`crate::ibe::combine`] — derives `indices` from the partials themselves).
pub fn lagrange_at_zero(indices: &[ShareIndex], i: ShareIndex) -> Fr {
    let xi = Fr::from(i);
    let mut num = Fr::one();
    let mut den = Fr::one();
    for &j in indices {
        if j == i {
            continue;
        }
        let xj = Fr::from(j);
        num *= -xj; // (0 - x_j)
        den *= xi - xj; // (x_i - x_j)
    }
    num * den
        .inverse()
        .expect("distinct indices => nonzero denominator")
}

/// Reconstruct the secret `f(0)` from `>= t` shares (dealer self-check / tests).
pub fn reconstruct_secret(shares: &[Share]) -> Fr {
    let indices: Vec<ShareIndex> = shares.iter().map(|s| s.index).collect();
    shares.iter().fold(Fr::zero(), |acc, s| {
        acc + lagrange_at_zero(&indices, s.index) * s.value
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_std::rand::{rngs::StdRng, SeedableRng};

    fn csprng() -> StdRng {
        // Deterministic *and* CryptoRng (StdRng is a CSPRNG) — reproducible tests.
        StdRng::seed_from_u64(0x5A6E_5A6E)
    }

    #[test]
    fn any_t_of_n_reconstructs_the_secret() {
        let mut rng = csprng();
        let secret = Fr::rand(&mut rng);
        let (t, n) = (3usize, 5usize);
        let shares = split(secret, t, n, &mut rng).unwrap();
        assert_eq!(shares.len(), n);

        assert_eq!(reconstruct_secret(&shares[0..t]), secret);
        let subset = vec![shares[1], shares[3], shares[4]];
        assert_eq!(reconstruct_secret(&subset), secret);
        assert_eq!(reconstruct_secret(&shares), secret);
    }

    #[test]
    fn fewer_than_t_does_not_reveal_secret() {
        let mut rng = csprng();
        let secret = Fr::rand(&mut rng);
        let shares = split(secret, 3, 5, &mut rng).unwrap();
        assert_ne!(reconstruct_secret(&shares[0..2]), secret);
    }

    #[test]
    fn lagrange_coeffs_sum_to_one() {
        let indices = [1u64, 2, 5, 7];
        let sum: Fr = indices
            .iter()
            .fold(Fr::zero(), |acc, &i| acc + lagrange_at_zero(&indices, i));
        assert_eq!(sum, Fr::one());
    }

    #[test]
    fn bad_threshold_is_rejected() {
        let mut rng = csprng();
        let secret = Fr::rand(&mut rng);
        assert_eq!(
            split(secret, 0, 5, &mut rng),
            Err(ShamirError::BadThreshold { t: 0, n: 5 })
        );
        assert_eq!(
            split(secret, 6, 5, &mut rng),
            Err(ShamirError::BadThreshold { t: 6, n: 5 })
        );
    }
}
