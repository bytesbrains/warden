//! Convenience builder for the Veil release condition (a Maktub Beat's `executed` flag).
//!
//! `MaktubCore` exposes no `executed(uint256)` getter — execution status is the 8th field
//! (index 7) of `getHeartbeat(uint256)`'s return tuple — so the condition reads `word: 7`.
//! Arbitrary conditions can instead be supplied as a JSON file (`--condition <file>`).

use serde_json::json;
use warden_core::condition::{Condition, Meta, Test};

/// Base Sepolia chain id (the Phase 0 condition source).
pub const CHAIN_ID: u64 = 84532;
/// Deployed `MaktubCore` on Base Sepolia (deployments/base-sepolia.json, 2026-06-16 stack).
pub const DEFAULT_CORE: &str = "0xb603C96D089F64Ac487EE0bdaE97D49848F86133";

/// `MaktubCore.getHeartbeat(beatId).executed == true` on Base Sepolia.
pub fn beat_executed(core_addr: &str, beat_id: &str, finality: u64) -> Condition {
    Condition::Contract {
        chain: CHAIN_ID,
        address: core_addr.to_string(),
        func: "getHeartbeat(uint256)".to_string(),
        args: vec![beat_id.to_string()],
        word: 7,
        test: Test {
            cmp: "==".to_string(),
            value: json!(true),
        },
        meta: Meta { finality, tier: 1 },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beat_condition_validates_and_targets_executed() {
        let c = beat_executed(DEFAULT_CORE, "42", 32);
        c.validate().unwrap();
        match c {
            Condition::Contract { word, func, .. } => {
                assert_eq!(word, 7);
                assert_eq!(func, "getHeartbeat(uint256)");
            }
            _ => panic!("expected contract"),
        }
    }
}
