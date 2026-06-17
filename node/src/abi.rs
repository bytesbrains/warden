//! Minimal ABI encoding/decoding for the conditions the PoC watcher evaluates.
//!
//! Scope: a function selector + `uint256` arguments (covers `executed(uint256)` and any
//! getter taking uint256 args), and selecting + comparing one 32-byte return word — including
//! a single **static** field of a tuple return (its ABI head word; see [`word_at`]). Dynamic
//! return types (`bytes`/arrays) and encoding non-uint256 args are out of scope for Phase 0.

use sha3::{Digest, Keccak256};

/// Errors from ABI encoding / decoding.
#[derive(thiserror::Error, Debug)]
pub enum AbiError {
    #[error("uint256 argument is not a non-negative decimal integer: {0:?}")]
    BadUint(String),
    #[error("uint256 argument overflows 256 bits: {0:?}")]
    Overflow(String),
    #[error("call return too short for the requested word: {0} bytes")]
    ShortReturn(usize),
}

/// The 4-byte selector `keccak256(signature)[..4]` for e.g. `"executed(uint256)"`.
pub fn selector(signature: &str) -> [u8; 4] {
    let mut h = Keccak256::new();
    h.update(signature.as_bytes());
    let digest = h.finalize();
    [digest[0], digest[1], digest[2], digest[3]]
}

/// Encode a non-negative decimal string as a big-endian 32-byte `uint256` word.
///
/// Done by hand (no big-int dependency): accumulate `acc = acc*10 + digit` directly in the
/// 32-byte array, rejecting non-digits and >256-bit overflow.
pub fn uint256_dec_to_be32(s: &str) -> Result<[u8; 32], AbiError> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(AbiError::BadUint(s.to_string()));
    }
    let mut acc = [0u8; 32];
    for d in s.bytes().map(|b| b - b'0') {
        // acc = acc * 10 + d, big-endian, with carry; any carry out of the top byte overflows.
        let mut carry = d as u16;
        for byte in acc.iter_mut().rev() {
            let v = (*byte as u16) * 10 + carry;
            *byte = (v & 0xff) as u8;
            carry = v >> 8;
        }
        if carry != 0 {
            return Err(AbiError::Overflow(s.to_string()));
        }
    }
    Ok(acc)
}

/// `selector(sig) ‖ arg0 ‖ arg1 ‖ …` — the `data` field of an `eth_call`. Each arg is a
/// decimal `uint256`.
pub fn encode_call(signature: &str, args: &[String]) -> Result<Vec<u8>, AbiError> {
    let mut data = selector(signature).to_vec();
    for a in args {
        data.extend_from_slice(&uint256_dec_to_be32(a)?);
    }
    Ok(data)
}

/// The `idx`-th 32-byte word of a return blob (0-based), big-endian. For a tuple getter this
/// is the `idx`-th return value's ABI head word — where a static value (`uint256`/`bool`/
/// `address`) sits inline, regardless of earlier dynamic (`bytes`/array) fields.
pub fn word_at(ret: &[u8], idx: u32) -> Result<[u8; 32], AbiError> {
    // Checked so a large `idx` can't wrap `usize` on a 32-bit target (a wrap could turn an
    // out-of-bounds read into a valid-looking offset). On overflow there is no such word.
    let start = (idx as usize)
        .checked_mul(32)
        .ok_or(AbiError::ShortReturn(ret.len()))?;
    let end = start
        .checked_add(32)
        .ok_or(AbiError::ShortReturn(ret.len()))?;
    if ret.len() < end {
        return Err(AbiError::ShortReturn(ret.len()));
    }
    let mut w = [0u8; 32];
    w.copy_from_slice(&ret[start..end]);
    Ok(w)
}

/// Apply a comparator (`==`, `!=`, `>=`, `<=`, `>`, `<`) between two big-endian 256-bit
/// words, treated as unsigned (lexicographic byte order == numeric order). Unknown
/// comparator → `false` (conditions are pre-validated, so this is defensive).
pub fn cmp_words(a: &[u8; 32], b: &[u8; 32], cmp: &str) -> bool {
    use std::cmp::Ordering::*;
    let ord = a.as_slice().cmp(b.as_slice());
    match cmp {
        "==" => ord == Equal,
        "!=" => ord != Equal,
        ">=" => ord != Less,
        "<=" => ord != Greater,
        ">" => ord == Greater,
        "<" => ord == Less,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_matches_known_values() {
        // keccak256("executed(uint256)")[..4]
        assert_eq!(hex::encode(selector("executed(uint256)")), "d3ecebd7");
        // transfer(address,uint256) — the canonical reference selector.
        assert_eq!(
            hex::encode(selector("transfer(address,uint256)")),
            "a9059cbb"
        );
    }

    #[test]
    fn dec_to_be32_basics() {
        assert_eq!(uint256_dec_to_be32("0").unwrap(), [0u8; 32]);
        let mut one = [0u8; 32];
        one[31] = 1;
        assert_eq!(uint256_dec_to_be32("1").unwrap(), one);
        let mut e256 = [0u8; 32];
        e256[30] = 1; // 256 = 0x0100
        assert_eq!(uint256_dec_to_be32("256").unwrap(), e256);
    }

    #[test]
    fn dec_to_be32_max_and_overflow() {
        let max = "115792089237316195423570985008687907853269984665640564039457584007913129639935";
        assert_eq!(uint256_dec_to_be32(max).unwrap(), [0xffu8; 32]);
        let over = "115792089237316195423570985008687907853269984665640564039457584007913129639936";
        assert!(matches!(
            uint256_dec_to_be32(over),
            Err(AbiError::Overflow(_))
        ));
        assert!(matches!(
            uint256_dec_to_be32("12x"),
            Err(AbiError::BadUint(_))
        ));
        assert!(matches!(uint256_dec_to_be32(""), Err(AbiError::BadUint(_))));
    }

    #[test]
    fn encode_call_layout() {
        let data = encode_call("executed(uint256)", &["1".to_string()]).unwrap();
        assert_eq!(data.len(), 4 + 32);
        assert_eq!(hex::encode(&data[..4]), "d3ecebd7");
        assert_eq!(data[35], 1); // last byte of the single arg word
    }

    #[test]
    fn word_at_selects_the_right_return_word() {
        // Two return words: [0]=7, [1]=1 (e.g. a tuple getter's 2nd field == true).
        let mut ret = vec![0u8; 64];
        ret[31] = 7;
        ret[63] = 1;
        assert_eq!(word_at(&ret, 0).unwrap()[31], 7);
        assert_eq!(word_at(&ret, 1).unwrap()[31], 1);
        assert!(matches!(word_at(&ret, 2), Err(AbiError::ShortReturn(64))));
    }

    #[test]
    fn cmp_words_unsigned_order() {
        let mut one = [0u8; 32];
        one[31] = 1;
        let zero = [0u8; 32];
        assert!(cmp_words(&one, &zero, ">"));
        assert!(cmp_words(&one, &one, "=="));
        assert!(cmp_words(&zero, &one, "<"));
        assert!(!cmp_words(&one, &zero, "<"));
        assert!(!cmp_words(&one, &zero, "??")); // unknown comparator
    }
}
