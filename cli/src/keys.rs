//! Recipient secp256k1 keys (the ECIES recipient gate). Reuses `warden-core`'s `ecies`
//! key types. Private key = 32-byte hex; public key = compressed SEC1 hex.

use rand::rngs::OsRng;
use warden_core::ecies::{PublicKey, SecretKey};

/// Generate a fresh recipient keypair.
pub fn generate() -> (SecretKey, PublicKey) {
    let sk = SecretKey::random(&mut OsRng);
    let pk = sk.public_key();
    (sk, pk)
}

pub fn secret_to_hex(sk: &SecretKey) -> String {
    hex::encode(sk.to_bytes())
}

pub fn public_to_hex(pk: &PublicKey) -> String {
    hex::encode(pk.to_sec1_bytes())
}

pub fn secret_from_hex(s: &str) -> Result<SecretKey, String> {
    let bytes = hex::decode(s.trim()).map_err(|e| format!("secret key hex: {e}"))?;
    SecretKey::from_slice(&bytes).map_err(|e| format!("invalid secret key: {e}"))
}

pub fn public_from_hex(s: &str) -> Result<PublicKey, String> {
    let bytes = hex::decode(s.trim()).map_err(|e| format!("public key hex: {e}"))?;
    PublicKey::from_sec1_bytes(&bytes).map_err(|e| format!("invalid public key: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_round_trip_through_hex() {
        let (sk, pk) = generate();
        let sk2 = secret_from_hex(&secret_to_hex(&sk)).unwrap();
        let pk2 = public_from_hex(&public_to_hex(&pk)).unwrap();
        assert_eq!(sk.to_bytes(), sk2.to_bytes());
        assert_eq!(pk, pk2);
        // The public key recovered from hex matches the secret's own public key.
        assert_eq!(sk2.public_key(), pk2);
    }

    #[test]
    fn rejects_garbage() {
        assert!(secret_from_hex("zz").is_err());
        assert!(public_from_hex("00").is_err());
    }
}
