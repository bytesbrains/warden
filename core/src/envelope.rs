//! The `warden-v1` envelope — the **double-wrap** (Veil's ciphertext).
//!
//! Composes three layers so the payload is readable only when the **condition holds**
//! (threshold IBE release) **and** by the **recipient** (their secp256k1 key):
//!
//! ```text
//! K          = random 32-byte content key
//! inner.ct   = AEAD_K(pad(payload), aad=inner)  // content — bucket-padded; AAD-bound
//! K_wrapped  = ECIES(K, recipientPub)           // recipient gate
//! obk        = random 32-byte outer key
//! outer.ibe  = IBE(obk, H(condition))           // condition gate (32-byte obk fits the IBE block)
//! outer.seal = AEAD_obk(K_wrapped, aad=outer)   // obk seals the ECIES-wrapped content key
//! ```
//!
//! Open: release `obk` via the threshold IBE → unseal `K_wrapped` → recipient ECIES-opens
//! `K` → AEAD-open + unpad `inner.ct`. Both gates are required: before the condition, `obk`
//! is unavailable so `K_wrapped` stays sealed and the content is unobtainable — even by the
//! recipient.
//!
//! **Hardening:** both AEAD layers bind `domain ‖ network ‖ H(condition)` as **associated
//! data** (tamper → explicit failure + domain separation), and the payload is **bucket-
//! padded** so ciphertext length doesn't leak payload size. Wire form: JSON (`condition`
//! nested, blobs hex). AEAD = ChaCha20-Poly1305 (`nonce ‖ ct`). ⚠️ PoC; not audited.

use ark_bls12_381::G1Projective;
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use ark_std::rand::{CryptoRng, Rng};
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    ChaCha20Poly1305, Nonce,
};
use serde::{Deserialize, Serialize};

use crate::condition::{self, Condition};
use crate::ecies::{self, PublicKey, SecretKey};
use crate::ibe::{self, MasterPublicKey};

const ALG: &str = "warden-v1";
const NONCE_LEN: usize = 12;
const AAD_OUTER: &[u8] = b"warden-v1-aad-O"; // domain tag for the condition-gated layer
const AAD_INNER: &[u8] = b"warden-v1-aad-I"; // domain tag for the content layer
/// Length-hiding buckets (bytes). Payloads (+4-byte length prefix) pad up to one of these,
/// or to a multiple of the largest. Provisional — freeze with vectors before mainnet (#184).
const PAD_BUCKETS: [usize; 6] = [64, 256, 1024, 4096, 16384, 65536];

/// The `warden-v1` ciphertext.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Envelope {
    pub alg: String,
    /// Which Warden federation / master key this is sealed to (bound into both AEAD layers).
    pub network: String,
    /// Public release condition (hashed into the IBE identity and the AEAD associated data).
    pub condition: Condition,
    pub outer: Outer,
    pub inner: Inner,
}

/// Condition-gated layer.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Outer {
    /// Hex of the canonical-serialized IBE ciphertext of `obk`.
    pub ibe: String,
    /// Hex of `nonce ‖ AEAD_obk(K_wrapped)`.
    pub seal: String,
}

/// Recipient-gated content layer. (No recipient metadata is stored — the recipient is
/// implicit in who can ECIES-open the content key.)
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Inner {
    /// Hex of `nonce ‖ AEAD_K(pad(payload))`.
    pub ct: String,
}

/// Errors from sealing / opening an envelope.
#[derive(thiserror::Error, Debug)]
pub enum EnvelopeError {
    #[error("unsupported envelope alg")]
    BadVersion,
    #[error("hex decode: {0}")]
    Hex(#[from] hex::FromHexError),
    #[error("malformed ciphertext")]
    Malformed,
    #[error("condition: {0}")]
    Condition(#[from] condition::Error),
    #[error("IBE key not released for this condition (or wrong key)")]
    NotReleased,
    #[error("AEAD authentication failed (tampered, wrong key, or wrong context)")]
    Aead,
    #[error("ecies: {0}")]
    Ecies(#[from] ecies::EciesError),
    #[error("content key has wrong length")]
    BadKeyLen,
}

/// Associated data binding a layer to its `domain`, the `network`, and the condition identity.
fn aad(domain: &[u8], network: &str, identity: &[u8; 32]) -> Vec<u8> {
    let net = network.as_bytes();
    let mut a = Vec::with_capacity(domain.len() + 4 + net.len() + 32);
    a.extend_from_slice(domain);
    a.extend_from_slice(&(net.len() as u32).to_le_bytes()); // length-prefix → unambiguous
    a.extend_from_slice(net);
    a.extend_from_slice(identity);
    a
}

fn next_bucket(n: usize) -> usize {
    for &b in &PAD_BUCKETS {
        if n <= b {
            return b;
        }
    }
    let largest = PAD_BUCKETS[PAD_BUCKETS.len() - 1];
    n.div_ceil(largest) * largest
}

/// `len(payload) (u32 LE) ‖ payload ‖ zero-pad` up to the next bucket — hides exact length.
fn pad(payload: &[u8]) -> Vec<u8> {
    let total = next_bucket(4 + payload.len());
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    out.resize(total, 0);
    out
}

fn unpad(padded: &[u8]) -> Result<Vec<u8>, EnvelopeError> {
    if padded.len() < 4 {
        return Err(EnvelopeError::Malformed);
    }
    let len = u32::from_le_bytes(padded[..4].try_into().expect("4 bytes")) as usize;
    if 4 + len > padded.len() {
        return Err(EnvelopeError::Malformed);
    }
    Ok(padded[4..4 + len].to_vec())
}

fn aead_seal<R: Rng>(
    key: &[u8; 32],
    pt: &[u8],
    associated: &[u8],
    rng: &mut R,
) -> Result<Vec<u8>, EnvelopeError> {
    let nonce: [u8; NONCE_LEN] = rng.gen();
    let ct = ChaCha20Poly1305::new_from_slice(key)
        .expect("32-byte key")
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: pt,
                aad: associated,
            },
        )
        .map_err(|_| EnvelopeError::Aead)?;
    let mut out = Vec::with_capacity(NONCE_LEN + ct.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(out)
}

fn aead_open(key: &[u8; 32], data: &[u8], associated: &[u8]) -> Result<Vec<u8>, EnvelopeError> {
    if data.len() < NONCE_LEN {
        return Err(EnvelopeError::Malformed);
    }
    let (nonce, ct) = data.split_at(NONCE_LEN);
    ChaCha20Poly1305::new_from_slice(key)
        .expect("32-byte key")
        .decrypt(
            Nonce::from_slice(nonce),
            Payload {
                msg: ct,
                aad: associated,
            },
        )
        .map_err(|_| EnvelopeError::Aead)
}

fn ser_ibe(ct: &ibe::Ciphertext) -> Vec<u8> {
    let mut b = Vec::new();
    ct.serialize_compressed(&mut b)
        .expect("serialize IBE ciphertext");
    b
}

/// Seal `payload` into a `warden-v1` envelope. `rng` must be a CSPRNG.
pub fn seal<R: Rng + CryptoRng>(
    condition: Condition,
    recipient_pub: &PublicKey,
    master_pub: &MasterPublicKey,
    network: &str,
    payload: &[u8],
    rng: &mut R,
) -> Result<Envelope, EnvelopeError> {
    let id = condition.identity()?;

    // Content layer: pad (length hiding) + AAD-bound AEAD.
    let k: [u8; 32] = rng.gen();
    let inner_ct = aead_seal(&k, &pad(payload), &aad(AAD_INNER, network, &id), rng)?;

    // Recipient gate.
    let k_wrapped = ecies::encrypt(recipient_pub, &k, rng);

    // Condition gate (hybrid: IBE wraps a 32-byte obk; obk seals K_wrapped).
    let obk: [u8; 32] = rng.gen();
    let ibe_ct = ibe::encrypt(master_pub, &id, &obk, rng);
    let outer_seal = aead_seal(&obk, &k_wrapped, &aad(AAD_OUTER, network, &id), rng)?;

    Ok(Envelope {
        alg: ALG.to_string(),
        network: network.to_string(),
        condition,
        outer: Outer {
            ibe: hex::encode(ser_ibe(&ibe_ct)),
            seal: hex::encode(outer_seal),
        },
        inner: Inner {
            ct: hex::encode(inner_ct),
        },
    })
}

/// Open an envelope given the released IBE key `d_id` (from `combine_verified`) and the
/// recipient secret key. Returns the payload. The condition / network / alg are bound into
/// the AEAD, so any tampering of the envelope metadata fails closed.
pub fn open(
    env: &Envelope,
    d_id: &G1Projective,
    recipient_priv: &SecretKey,
) -> Result<Vec<u8>, EnvelopeError> {
    if env.alg != ALG {
        return Err(EnvelopeError::BadVersion);
    }
    let id = env.condition.identity()?;

    // Condition gate: IBE-decrypt obk.
    let ibe_bytes = hex::decode(&env.outer.ibe)?;
    let ibe_ct = ibe::Ciphertext::deserialize_compressed(ibe_bytes.as_slice())
        .map_err(|_| EnvelopeError::Malformed)?;
    let obk = ibe::decrypt(d_id, &ibe_ct).ok_or(EnvelopeError::NotReleased)?;

    // Unseal the ECIES-wrapped content key.
    let k_wrapped = aead_open(
        &obk,
        &hex::decode(&env.outer.seal)?,
        &aad(AAD_OUTER, &env.network, &id),
    )?;

    // Recipient gate: ECIES-open the content key.
    let k: [u8; 32] = ecies::decrypt(recipient_priv, &k_wrapped)?
        .try_into()
        .map_err(|_| EnvelopeError::BadKeyLen)?;

    // Content.
    let padded = aead_open(
        &k,
        &hex::decode(&env.inner.ct)?,
        &aad(AAD_INNER, &env.network, &id),
    )?;
    unpad(&padded)
}

#[cfg(all(test, feature = "trusted-dealer"))]
mod tests {
    use super::*;
    use crate::condition::{Meta, Test};
    use crate::dealer::deal;
    use crate::ibe::{combine_verified, partial};
    use ark_std::rand::{rngs::StdRng, SeedableRng};
    use serde_json::json;

    fn beat(beat_id: &str) -> Condition {
        Condition::Contract {
            chain: 8453,
            address: "0x00".into(),
            func: "executed(uint256)".into(),
            args: vec![beat_id.into()],
            word: 0,
            test: Test {
                cmp: "==".into(),
                value: json!(true),
            },
            meta: Meta {
                finality: 32,
                tier: 1,
            },
        }
    }

    fn release(fed: &crate::dealer::DealerOutput, id: &[u8], idxs: [usize; 3]) -> G1Projective {
        let partials: Vec<_> = idxs.iter().map(|&i| partial(&fed.shares[i], id)).collect();
        combine_verified(&partials, id, &fed.share_pubkeys).unwrap()
    }

    #[test]
    fn full_double_wrap_round_trip() {
        let mut rng = StdRng::seed_from_u64(1);
        let fed = deal(3, 5, &mut rng).unwrap();
        let recipient = SecretKey::random(&mut rng);
        let cond = beat("42");
        let payload = b"the will: seed phrase abandon abandon ... art";

        let env = seal(
            cond.clone(),
            &recipient.public_key(),
            &fed.mpk,
            "testnet",
            payload,
            &mut rng,
        )
        .unwrap();

        let d_id = release(&fed, &cond.identity().unwrap(), [0, 2, 4]);
        assert_eq!(open(&env, &d_id, &recipient).unwrap(), payload);

        // JSON wire round-trip, then open.
        let json = serde_json::to_string(&env).unwrap();
        let back: Envelope = serde_json::from_str(&json).unwrap();
        assert_eq!(open(&back, &d_id, &recipient).unwrap(), payload);
    }

    #[test]
    fn wrong_recipient_key_fails_even_with_release() {
        let mut rng = StdRng::seed_from_u64(2);
        let fed = deal(3, 5, &mut rng).unwrap();
        let recipient = SecretKey::random(&mut rng);
        let cond = beat("42");
        let env = seal(
            cond.clone(),
            &recipient.public_key(),
            &fed.mpk,
            "t",
            b"secret",
            &mut rng,
        )
        .unwrap();
        let d_id = release(&fed, &cond.identity().unwrap(), [0, 1, 2]);

        let attacker = SecretKey::random(&mut rng);
        assert!(open(&env, &d_id, &attacker).is_err());
    }

    #[test]
    fn key_for_other_condition_does_not_release() {
        let mut rng = StdRng::seed_from_u64(3);
        let fed = deal(3, 5, &mut rng).unwrap();
        let recipient = SecretKey::random(&mut rng);
        let cond = beat("42");
        let env = seal(
            cond,
            &recipient.public_key(),
            &fed.mpk,
            "t",
            b"secret",
            &mut rng,
        )
        .unwrap();

        let wrong = release(&fed, &beat("43").identity().unwrap(), [0, 1, 2]);
        assert!(matches!(
            open(&env, &wrong, &recipient),
            Err(EnvelopeError::NotReleased)
        ));
    }

    #[test]
    fn tampered_network_is_rejected_by_aad() {
        let mut rng = StdRng::seed_from_u64(4);
        let fed = deal(3, 5, &mut rng).unwrap();
        let recipient = SecretKey::random(&mut rng);
        let cond = beat("42");
        let mut env = seal(
            cond.clone(),
            &recipient.public_key(),
            &fed.mpk,
            "honest",
            b"secret",
            &mut rng,
        )
        .unwrap();
        let d_id = release(&fed, &cond.identity().unwrap(), [0, 1, 2]);

        env.network = "evil".into(); // not authenticated by the IBE, but bound as AEAD AAD
        assert!(matches!(
            open(&env, &d_id, &recipient),
            Err(EnvelopeError::Aead)
        ));
    }

    #[test]
    fn padding_hides_length_and_round_trips() {
        let mut rng = StdRng::seed_from_u64(5);
        let fed = deal(3, 5, &mut rng).unwrap();
        let recipient = SecretKey::random(&mut rng);
        let cond = beat("42");
        let id = cond.identity().unwrap();
        let d_id = release(&fed, &id, [0, 1, 2]);

        // Two very different small payloads land in the same bucket → identical inner.ct length.
        let e1 = seal(
            cond.clone(),
            &recipient.public_key(),
            &fed.mpk,
            "t",
            b"a",
            &mut rng,
        )
        .unwrap();
        let e2 = seal(
            cond.clone(),
            &recipient.public_key(),
            &fed.mpk,
            "t",
            &[b'x'; 50],
            &mut rng,
        )
        .unwrap();
        assert_eq!(e1.inner.ct.len(), e2.inner.ct.len());

        // A larger payload still round-trips.
        let big = vec![7u8; 5000];
        let e3 = seal(cond, &recipient.public_key(), &fed.mpk, "t", &big, &mut rng).unwrap();
        assert_eq!(open(&e3, &d_id, &recipient).unwrap(), big);
    }
}
