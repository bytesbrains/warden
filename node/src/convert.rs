//! Convert a condition's JSON `test`/`value` into something comparable against a chain read.
//!
//! Split out of [`crate::eval`] to keep that (security-critical) module focused on the
//! release decision. These are pure functions with no chain access.

use serde_json::Value;

use crate::abi::uint256_dec_to_be32;
use crate::eval::EvalError;

/// A `contract` test value → a 32-byte big-endian word: `bool` (true⇒1), a decimal-string or
/// integer `uint256`. Floats / negatives / other shapes are rejected.
pub(crate) fn value_to_word(v: &Value) -> Result<[u8; 32], EvalError> {
    match v {
        Value::Bool(b) => {
            let mut w = [0u8; 32];
            w[31] = u8::from(*b);
            Ok(w)
        }
        Value::String(s) => Ok(uint256_dec_to_be32(s)?),
        Value::Number(n) if n.is_u64() => Ok(uint256_dec_to_be32(&n.to_string())?),
        other => Err(EvalError::Validation(format!(
            "unsupported contract test value: {other}"
        ))),
    }
}

/// A `block` field value → `u64` (block number / timestamp). Accepts a JSON integer or a
/// decimal string.
pub(crate) fn value_to_u64(v: &Value) -> Result<u64, EvalError> {
    match v {
        Value::Number(n) if n.is_u64() => Ok(n.as_u64().unwrap()),
        Value::String(s) => s
            .parse()
            .map_err(|_| EvalError::Validation(format!("block value not a u64: {s:?}"))),
        other => Err(EvalError::Validation(format!(
            "unsupported block value: {other}"
        ))),
    }
}

/// Apply a comparator between two `u64`s. Unknown comparator → `false` (pre-validated).
pub(crate) fn cmp_u64(a: u64, b: u64, cmp: &str) -> bool {
    match cmp {
        "==" => a == b,
        "!=" => a != b,
        ">=" => a >= b,
        "<=" => a <= b,
        ">" => a > b,
        "<" => a < b,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn value_conversions() {
        assert_eq!(value_to_word(&json!(true)).unwrap()[31], 1);
        assert_eq!(value_to_word(&json!(false)).unwrap(), [0u8; 32]);
        assert_eq!(value_to_word(&json!("256")).unwrap()[30], 1);
        assert!(value_to_word(&json!(1.5)).is_err());
        assert_eq!(value_to_u64(&json!(42)).unwrap(), 42);
        assert_eq!(value_to_u64(&json!("42")).unwrap(), 42);
        assert!(value_to_u64(&json!(true)).is_err());
        assert!(cmp_u64(5, 3, ">"));
        assert!(!cmp_u64(5, 3, "<"));
        assert!(!cmp_u64(5, 5, "??"));
    }
}
