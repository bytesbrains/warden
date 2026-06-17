//! Condition evaluation — the security-critical core (issue #181 WS-C; spec
//! `docs/02-condition-model.md`, `docs/03-protocol.md` §3/§5).
//!
//! A node releases its partial **only** if the condition holds against **finalized** chain
//! state. This module decides that yes/no. Guarantees it enforces:
//!
//! - **Determinism** — reads at a fixed finality tag (`finalized` by default), never live
//!   state, so every honest node reaches the same answer.
//! - **Chain binding** — refuses a condition whose `chain` isn't the one this node watches
//!   (it must not evaluate a chain it can't see).
//! - **Tier gate** — Phase 0 evaluates **tier-1** on-chain state only (`contract`/`block`);
//!   tier-2 (oracle/API) is refused.
//! - **Fail-closed** — any RPC failure is a *transient* error (the caller retries); the node
//!   never releases on an unreadable or ambiguous chain.
//!
//! Compound (`all`/`any`/`not`/`threshold`) and `event` conditions are refused in Phase 0.
//! Monotonicity/anchoring (`docs/02` §2) is the app's responsibility — the node evaluates
//! the condition as written at the finalized head.

use warden_core::condition::Condition;

use crate::abi::{self, cmp_words, encode_call, word_at};
use crate::config::FinalityTag;
use crate::convert::{cmp_u64, value_to_u64, value_to_word};
use crate::rpc::{RpcClient, RpcError};

/// Why an evaluation could not yield a release. Distinguishes **permanent** client errors
/// (bad/unsupported condition → HTTP 4xx, don't retry) from **transient** ones (RPC down →
/// HTTP 5xx, retry).
#[derive(thiserror::Error, Debug)]
pub enum EvalError {
    #[error("condition validation: {0}")]
    Validation(String),
    #[error("unsupported condition: {0}")]
    Unsupported(String),
    #[error("abi: {0}")]
    Abi(#[from] abi::AbiError),
    #[error("rpc: {0}")]
    Rpc(#[from] RpcError),
}

impl EvalError {
    /// True if the client should retry later (the chain was unreadable, not the request bad).
    pub fn is_transient(&self) -> bool {
        matches!(self, EvalError::Rpc(_))
    }
}

/// Evaluate `cond` against finalized chain state. `Ok(true)` ⇒ release the partial.
pub fn evaluate(
    cond: &Condition,
    rpc: &RpcClient,
    chain_id: u64,
    tag: FinalityTag,
) -> Result<bool, EvalError> {
    cond.validate()
        .map_err(|e| EvalError::Validation(e.to_string()))?;
    check_chain_and_tier(cond, chain_id)?;

    match cond {
        Condition::Contract {
            address,
            func,
            args,
            word,
            test,
            ..
        } => {
            let data = encode_call(func, args)?;
            let ret = rpc.eth_call(address, &data, tag.as_rpc())?;
            let got = word_at(&ret, *word)?;
            let expected = value_to_word(&test.value)?;
            Ok(cmp_words(&got, &expected, &test.cmp))
        }
        Condition::Block {
            field, cmp, value, ..
        } => {
            if field != "number" && field != "timestamp" {
                return Err(EvalError::Validation(format!(
                    "unknown block field {field:?}"
                )));
            }
            let got = rpc.block_field(field, tag.as_rpc())?;
            let expected = value_to_u64(value)?;
            Ok(cmp_u64(got, expected, cmp))
        }
        Condition::Event { .. } => Err(EvalError::Unsupported("event (Phase 0)".into())),
        Condition::All { .. }
        | Condition::Any { .. }
        | Condition::Not { .. }
        | Condition::Threshold { .. } => Err(EvalError::Unsupported(
            "compound condition (Phase 0)".into(),
        )),
    }
}

fn check_chain_and_tier(cond: &Condition, chain_id: u64) -> Result<(), EvalError> {
    let (chain, tier) = match cond {
        Condition::Contract { chain, meta, .. } => (*chain, meta.tier),
        Condition::Block { chain, meta, .. } => (*chain, meta.tier),
        _ => return Ok(()), // other variants are refused in evaluate()
    };
    if chain != chain_id {
        return Err(EvalError::Validation(format!(
            "condition chain {chain} != node chain {chain_id}"
        )));
    }
    if tier != 1 {
        return Err(EvalError::Unsupported(format!(
            "tier {tier} (this node evaluates tier-1 on-chain state only)"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use warden_core::condition::{Meta, Test};

    fn rpc() -> RpcClient {
        // Never actually called in these tests — they all fail before the RPC read.
        RpcClient::new("http://127.0.0.1:1")
    }

    fn contract(chain: u64, tier: u8, args: Vec<String>) -> Condition {
        Condition::Contract {
            chain,
            address: "0x000000000000000000000000000000000000dead".into(),
            func: "executed(uint256)".into(),
            args,
            word: 0,
            test: Test {
                cmp: "==".into(),
                value: json!(true),
            },
            meta: Meta { finality: 32, tier },
        }
    }

    #[test]
    fn rejects_wrong_chain() {
        let err = evaluate(
            &contract(1, 1, vec!["1".into()]),
            &rpc(),
            84532,
            FinalityTag::Finalized,
        )
        .unwrap_err();
        assert!(matches!(err, EvalError::Validation(_)) && !err.is_transient());
    }

    #[test]
    fn rejects_tier_2() {
        let err = evaluate(
            &contract(84532, 2, vec!["1".into()]),
            &rpc(),
            84532,
            FinalityTag::Finalized,
        )
        .unwrap_err();
        assert!(matches!(err, EvalError::Unsupported(_)));
    }

    #[test]
    fn rejects_bad_uint_arg_before_rpc() {
        let err = evaluate(
            &contract(84532, 1, vec!["not-a-number".into()]),
            &rpc(),
            84532,
            FinalityTag::Finalized,
        )
        .unwrap_err();
        assert!(matches!(err, EvalError::Abi(_)) && !err.is_transient());
    }

    #[test]
    fn rejects_compound() {
        let c = Condition::Any {
            of: vec![contract(84532, 1, vec!["1".into()])],
            meta: Meta {
                finality: 32,
                tier: 1,
            },
        };
        let err = evaluate(&c, &rpc(), 84532, FinalityTag::Finalized).unwrap_err();
        assert!(matches!(err, EvalError::Unsupported(_)));
    }
}
