//! Condition model + canonicalization + identity derivation.
//!
//! A Warden ciphertext's outer layer is IBE-encrypted to an **identity** that is the
//! domain-separated hash of a **canonical** serialization of the release condition:
//!
//! ```text
//! identity = SHA-256( "warden-cond-v1" || jcs(condition) )
//! ```
//!
//! The identity is recomputable by any node from the public `condition`, and it
//! cryptographically **binds** the condition to the ciphertext: a node will only
//! release its partial for `identity` after it verifies `H(condition) == identity`
//! *and* independently evaluates the condition true on-chain.
//!
//! ## Canonicalization
//! We serialize the condition through `serde_json::Value` and emit it with
//! `serde_json::to_string`, which — with **default features** — uses a `BTreeMap`-backed
//! map (sorted keys) and compact separators. For our constrained schema (no floats; all
//! `uint256` carried as decimal strings) this matches RFC 8785 (JCS). A full RFC-8785
//! implementation should be adopted before mainnet and a cross-language test vector pinned.
//!
//! ## Type discipline (load-bearing)
//! All `uint256` arguments/values are **decimal strings**, never JSON numbers — otherwise
//! a number-vs-string mismatch (or JS `2^53` precision loss) yields a *different* identity
//! and silently breaks decryption. This is enforced at the type level: `args: Vec<String>`.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Domain-separation tag mixed into every identity. Bumped if the canonical form changes.
pub const DOMAIN: &[u8] = b"warden-cond-v1";

/// Confirmation depth + trust tier carried by every condition.
///
/// `finality` is a *requested* depth; nodes enforce the **federation-wide floor** and may
/// only override it upward (see `warden/docs/03-protocol.md` §5).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Meta {
    /// Requested confirmation depth (federation floor applies; override upward only).
    pub finality: u64,
    /// Trust tier: 1 = on-chain state (deterministic); 2 = external data (opt-in).
    pub tier: u8,
}

/// A comparison test applied to a read value (`{ cmp, value }`).
///
/// `value` is an arbitrary JSON scalar (`true`, a number, or a decimal string for uint256).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Test {
    /// One of `== != >= <= > <` (validated by [`Condition::validate`]).
    pub cmp: String,
    pub value: serde_json::Value,
}

/// A release condition. Exactly one variant; serialized with an internal `type` tag.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Condition {
    /// Read a contract getter and compare its return value. Generalizes `executed(beatId)==true`.
    Contract {
        chain: u64,
        address: String,
        /// Solidity function selector signature, e.g. `executed(uint256)`. (`fn` is a keyword.)
        #[serde(rename = "fn")]
        func: String,
        /// Arguments; uint256 values are decimal **strings**.
        args: Vec<String>,
        /// Which 32-byte word of the ABI return to compare (0-based). `0` for a getter that
        /// returns a single value. For a **tuple** getter it is the target field's ordinal
        /// position — static fields sit inline in the ABI head regardless of earlier dynamic
        /// types, so no full tuple decode is needed. Example: `MaktubCore.getHeartbeat(uint256)`
        /// returns `(owner, recipients, payload, interval, lastCheckIn, createdAt, checkInCount,
        /// executed, deactivated)`, so the `executed` bool is `word: 7`.
        ///
        /// Omitted from the canonical form when `0`, so single-value getters hash identically
        /// to a condition written before this field existed.
        #[serde(default, skip_serializing_if = "is_zero")]
        word: u32,
        test: Test,
        meta: Meta,
    },
    /// Block-level predicate, e.g. `timestamp >= T` (a timelock).
    Block {
        chain: u64,
        /// `"number"` or `"timestamp"`.
        field: String,
        cmp: String,
        value: serde_json::Value,
        meta: Meta,
    },
    /// An event/log was emitted.
    Event {
        chain: u64,
        address: String,
        /// Event signature, e.g. `HeartbeatExecuted(uint256)`.
        sig: String,
        args: Vec<String>,
        meta: Meta,
    },
    /// All sub-conditions hold.
    All { of: Vec<Condition>, meta: Meta },
    /// Any sub-condition holds.
    Any { of: Vec<Condition>, meta: Meta },
    /// The (single) sub-condition does not hold. `of` must have length 1.
    Not { of: Vec<Condition>, meta: Meta },
    /// At least `k` of the sub-conditions hold.
    Threshold {
        k: u32,
        of: Vec<Condition>,
        meta: Meta,
    },
}

/// Errors from canonicalization / validation.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("serialization: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("invalid comparator: {0:?}")]
    BadComparator(String),
    #[error("`not` requires exactly one sub-condition, got {0}")]
    BadNotArity(usize),
    #[error("`threshold` k={0} exceeds number of sub-conditions {1}")]
    BadThresholdK(u32, usize),
}

const VALID_CMP: [&str; 6] = ["==", "!=", ">=", "<=", ">", "<"];

/// `skip_serializing_if` predicate so `word: 0` is omitted from the canonical form.
fn is_zero(n: &u32) -> bool {
    *n == 0
}

impl Condition {
    /// Canonical byte serialization (RFC-8785-style: sorted keys, compact).
    ///
    /// Goes through `serde_json::Value` so map keys are emitted sorted regardless of
    /// struct field order.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, Error> {
        let value = serde_json::to_value(self)?;
        let s = serde_json::to_string(&value)?; // default features => sorted keys, compact
        Ok(s.into_bytes())
    }

    /// The domain-separated 32-byte identity this condition's payload is IBE-encrypted to.
    pub fn identity(&self) -> Result<[u8; 32], Error> {
        let mut h = Sha256::new();
        h.update(DOMAIN);
        h.update(self.canonical_bytes()?);
        Ok(h.finalize().into())
    }

    /// Structural validation (comparators, `not`/`threshold` arity). Does **not** check chains.
    pub fn validate(&self) -> Result<(), Error> {
        match self {
            Condition::Contract { test, .. } => check_cmp(&test.cmp),
            Condition::Block { cmp, .. } => check_cmp(cmp),
            Condition::Event { .. } => Ok(()),
            Condition::All { of, .. } | Condition::Any { of, .. } => {
                of.iter().try_for_each(|c| c.validate())
            }
            Condition::Not { of, .. } => {
                if of.len() != 1 {
                    return Err(Error::BadNotArity(of.len()));
                }
                of[0].validate()
            }
            Condition::Threshold { k, of, .. } => {
                if *k as usize > of.len() {
                    return Err(Error::BadThresholdK(*k, of.len()));
                }
                of.iter().try_for_each(|c| c.validate())
            }
        }
    }
}

fn check_cmp(cmp: &str) -> Result<(), Error> {
    if VALID_CMP.contains(&cmp) {
        Ok(())
    } else {
        Err(Error::BadComparator(cmp.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_contract() -> Condition {
        Condition::Contract {
            chain: 8453,
            address: "0x00".into(),
            func: "executed(uint256)".into(),
            args: vec!["12345678901234567890".into()],
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

    #[test]
    fn canonical_is_sorted_and_compact() {
        // Keys must come out alphabetically sorted, nested objects too, no whitespace.
        // `word: 0` is omitted, so this matches a pre-`word` single-value condition exactly.
        let got = String::from_utf8(sample_contract().canonical_bytes().unwrap()).unwrap();
        let expected = r#"{"address":"0x00","args":["12345678901234567890"],"chain":8453,"fn":"executed(uint256)","meta":{"finality":32,"tier":1},"test":{"cmp":"==","value":true},"type":"contract"}"#;
        assert_eq!(got, expected);
    }

    #[test]
    fn nonzero_word_appears_in_canonical_and_changes_identity() {
        let base = sample_contract();
        let mut with_word = sample_contract();
        if let Condition::Contract { word, .. } = &mut with_word {
            *word = 7;
        }
        let canon = String::from_utf8(with_word.canonical_bytes().unwrap()).unwrap();
        assert!(
            canon.contains(r#""word":7"#),
            "word must appear when non-zero: {canon}"
        );
        // A different return-word selector is a different condition → different identity.
        assert_ne!(base.identity().unwrap(), with_word.identity().unwrap());
    }

    #[test]
    fn missing_word_deserializes_to_zero() {
        // A condition written without `word` (the documented single-value form) round-trips.
        let json = r#"{"type":"contract","chain":8453,"address":"0x00","fn":"executed(uint256)",
            "args":["1"],"test":{"cmp":"==","value":true},"meta":{"finality":32,"tier":1}}"#;
        let c: Condition = serde_json::from_str(json).unwrap();
        match c {
            Condition::Contract { word, .. } => assert_eq!(word, 0),
            _ => panic!("expected contract"),
        }
    }

    #[test]
    fn identity_is_deterministic() {
        let a = sample_contract().identity().unwrap();
        let b = sample_contract().identity().unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), 32);
    }

    #[test]
    fn identity_changes_with_any_field() {
        let base = sample_contract().identity().unwrap();
        let mut other = sample_contract();
        if let Condition::Contract { args, .. } = &mut other {
            args[0] = "999".into();
        }
        assert_ne!(base, other.identity().unwrap());
    }

    #[test]
    fn uint256_string_vs_number_differ() {
        // The whole point of mandating decimal strings: "1" (string) != 1 (number).
        let as_str = Condition::Block {
            chain: 8453,
            field: "number".into(),
            cmp: ">=".into(),
            value: json!("1"),
            meta: Meta {
                finality: 32,
                tier: 1,
            },
        };
        let as_num = Condition::Block {
            chain: 8453,
            field: "number".into(),
            cmp: ">=".into(),
            value: json!(1),
            meta: Meta {
                finality: 32,
                tier: 1,
            },
        };
        assert_ne!(as_str.identity().unwrap(), as_num.identity().unwrap());
    }

    #[test]
    fn round_trips_through_json() {
        let c = sample_contract();
        let s = serde_json::to_string(&c).unwrap();
        let back: Condition = serde_json::from_str(&s).unwrap();
        assert_eq!(c, back);
        assert_eq!(c.identity().unwrap(), back.identity().unwrap());
    }

    #[test]
    fn validate_rejects_bad_comparator_and_arity() {
        let mut bad = sample_contract();
        if let Condition::Contract { test, .. } = &mut bad {
            test.cmp = "≈".into();
        }
        assert!(bad.validate().is_err());

        let bad_not = Condition::Not {
            of: vec![],
            meta: Meta {
                finality: 1,
                tier: 1,
            },
        };
        assert!(matches!(bad_not.validate(), Err(Error::BadNotArity(0))));
    }

    #[test]
    fn compound_validates_recursively() {
        let c = Condition::All {
            of: vec![
                sample_contract(),
                Condition::Block {
                    chain: 8453,
                    field: "timestamp".into(),
                    cmp: ">=".into(),
                    value: json!(1893456000u64),
                    meta: Meta {
                        finality: 32,
                        tier: 1,
                    },
                },
            ],
            meta: Meta {
                finality: 32,
                tier: 1,
            },
        };
        assert!(c.validate().is_ok());
        assert_eq!(c.identity().unwrap().len(), 32);
    }
}
