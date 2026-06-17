//! ECIES over secp256k1 — the **recipient gate** of the Warden double-wrap.
//!
//! `encrypt(recipientPub, m)` → `ephPub ‖ nonce ‖ AEAD_k(m)` where
//! `k = HKDF-SHA256(ikm = ECDH_x, info = tag ‖ ephPub ‖ recipientPub)`. The HKDF binds
//! the derived key to **both** public keys (SEC1 / ISO-18033-2 shared-info), so the key is
//! tied to the specific exchange. Only the holder of the recipient private key can recover
//! `k`. AEAD = ChaCha20-Poly1305; a fresh ephemeral key (hence a fresh `k`) per message.
//!
//! Recipients use secp256k1 keys (aligned with Maktub's `RecipientRegistry`). ⚠️ PoC: the
//! HKDF `info` tag is provisional and the exact byte format must be reconciled with Maktub's
//! existing ECIES envelope and frozen with cross-language vectors before mainnet (#184).

use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};
use hkdf::Hkdf;
use k256::ecdh::diffie_hellman;
use k256::elliptic_curve::sec1::ToEncodedPoint;
pub use k256::{PublicKey, SecretKey};
use rand_core::{CryptoRng, RngCore};
use sha2::Sha256;

const COMPRESSED_PK_LEN: usize = 33;
const NONCE_LEN: usize = 12;
const HKDF_TAG: &[u8] = b"warden-ecies-secp256k1-v1";

/// Errors from ECIES decryption.
#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum EciesError {
    #[error("ciphertext too short")]
    Truncated,
    #[error("invalid ephemeral public key")]
    BadEphemeralKey,
    #[error("AEAD decryption failed")]
    Aead,
}

/// HKDF-SHA256, binding the shared secret to both public keys (anti key-confusion).
fn derive_key(shared_x: &[u8], eph_pub: &[u8], recipient_pub: &[u8]) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(None, shared_x);
    let mut info = Vec::with_capacity(HKDF_TAG.len() + eph_pub.len() + recipient_pub.len());
    info.extend_from_slice(HKDF_TAG);
    info.extend_from_slice(eph_pub);
    info.extend_from_slice(recipient_pub);
    let mut key = [0u8; 32];
    hk.expand(&info, &mut key)
        .expect("HKDF expand of 32 bytes is within output limits");
    key
}

/// Encrypt `plaintext` to `recipient_pub`. `rng` must be a CSPRNG. (Inputs are small —
/// content keys — so AEAD encryption cannot fail in practice.)
pub fn encrypt<R: RngCore + CryptoRng>(
    recipient_pub: &PublicKey,
    plaintext: &[u8],
    rng: &mut R,
) -> Vec<u8> {
    let eph = SecretKey::random(rng);
    let eph_pub = eph.public_key().to_encoded_point(true);
    let recipient_pub_pt = recipient_pub.to_encoded_point(true);
    let shared = diffie_hellman(eph.to_nonzero_scalar(), recipient_pub.as_affine());
    let key = derive_key(
        shared.raw_secret_bytes().as_slice(),
        eph_pub.as_bytes(),
        recipient_pub_pt.as_bytes(),
    );

    let mut nonce = [0u8; NONCE_LEN];
    rng.fill_bytes(&mut nonce);
    let ct = ChaCha20Poly1305::new_from_slice(&key)
        .expect("32-byte key")
        .encrypt(Nonce::from_slice(&nonce), plaintext)
        .expect("AEAD encryption is infallible for small content keys");

    let mut out = Vec::with_capacity(COMPRESSED_PK_LEN + NONCE_LEN + ct.len());
    out.extend_from_slice(eph_pub.as_bytes());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    out
}

/// Decrypt an ECIES blob with the recipient secret key.
pub fn decrypt(recipient_priv: &SecretKey, data: &[u8]) -> Result<Vec<u8>, EciesError> {
    if data.len() < COMPRESSED_PK_LEN + NONCE_LEN {
        return Err(EciesError::Truncated);
    }
    let (eph_pub_bytes, rest) = data.split_at(COMPRESSED_PK_LEN);
    let (nonce, ct) = rest.split_at(NONCE_LEN);

    let eph_pub =
        PublicKey::from_sec1_bytes(eph_pub_bytes).map_err(|_| EciesError::BadEphemeralKey)?;
    let recipient_pub_pt = recipient_priv.public_key().to_encoded_point(true);
    let shared = diffie_hellman(recipient_priv.to_nonzero_scalar(), eph_pub.as_affine());
    let key = derive_key(
        shared.raw_secret_bytes().as_slice(),
        eph_pub_bytes,
        recipient_pub_pt.as_bytes(),
    );

    ChaCha20Poly1305::new_from_slice(&key)
        .expect("32-byte key")
        .decrypt(Nonce::from_slice(nonce), ct)
        .map_err(|_| EciesError::Aead)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_std::rand::{rngs::StdRng, SeedableRng};

    fn csprng() -> StdRng {
        StdRng::seed_from_u64(0x_EC1E5)
    }

    #[test]
    fn round_trip() {
        let mut rng = csprng();
        let sk = SecretKey::random(&mut rng);
        let pk = sk.public_key();
        let msg = b"a 32-byte content key would go here..";
        let ct = encrypt(&pk, msg, &mut rng);
        assert_eq!(decrypt(&sk, &ct).unwrap(), msg);
    }

    #[test]
    fn wrong_key_fails() {
        let mut rng = csprng();
        let sk = SecretKey::random(&mut rng);
        let other = SecretKey::random(&mut rng);
        let ct = encrypt(&sk.public_key(), b"secret", &mut rng);
        assert_eq!(decrypt(&other, &ct), Err(EciesError::Aead));
    }

    #[test]
    fn tamper_and_truncation_rejected() {
        let mut rng = csprng();
        let sk = SecretKey::random(&mut rng);
        let mut ct = encrypt(&sk.public_key(), b"secret", &mut rng);
        let last = ct.len() - 1;
        ct[last] ^= 0xff;
        assert_eq!(decrypt(&sk, &ct), Err(EciesError::Aead));
        assert_eq!(decrypt(&sk, &[0u8; 4]), Err(EciesError::Truncated));
    }
}
