//! The `POST /partial` request → response logic, factored out of the server loop so it can
//! be unit-tested without a socket.
//!
//! Contract: the request carries the **full condition** (not a raw identity). The node
//! derives `identity = H(condition)` itself and signs *that* — it can never be tricked into
//! signing an attacker-chosen identity. It releases only when [`crate::eval::evaluate`]
//! returns `true` against finalized chain state.

use ark_serialize::CanonicalSerialize;
use serde::Deserialize;
use serde_json::{json, Value};

use warden_core::condition::Condition;
use warden_core::ibe::partial;
use warden_core::shamir::Share;

use crate::config::FinalityTag;
use crate::eval::evaluate;
use crate::rpc::RpcClient;

#[derive(Deserialize)]
struct PartialRequest {
    condition: Condition,
}

/// An HTTP status + JSON body.
pub struct Reply {
    pub status: u16,
    pub body: String,
}

fn reply(status: u16, v: Value) -> Reply {
    Reply {
        status,
        body: v.to_string(),
    }
}

/// Hex of an arkworks value's compressed canonical form (the released partial, the mpk).
pub(crate) fn canon_hex<T: CanonicalSerialize>(v: &T) -> String {
    let mut buf = Vec::new();
    v.serialize_compressed(&mut buf)
        .expect("canonical serialization to a Vec is infallible");
    hex::encode(buf)
}

/// Handle one `POST /partial` body. Status codes:
/// - `200 {released:true, …}` — condition holds; partial released.
/// - `200 {released:false, reason:"condition_not_met"}` — valid request, not yet true (retry later).
/// - `400` — malformed body / un-hashable condition.
/// - `422` — well-formed but unsupported/invalid condition (don't retry).
/// - `503` — chain unreadable (transient; retry).
pub fn handle_partial(
    body: &str,
    share: &Share,
    rpc: &RpcClient,
    chain_id: u64,
    tag: FinalityTag,
) -> Reply {
    let req: PartialRequest = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(e) => {
            return reply(
                400,
                json!({"released": false, "reason": "bad_request", "detail": e.to_string()}),
            )
        }
    };
    let cond = req.condition;
    let id = match cond.identity() {
        Ok(id) => id,
        Err(e) => {
            return reply(
                400,
                json!({"released": false, "reason": "bad_condition", "detail": e.to_string()}),
            )
        }
    };

    match evaluate(&cond, rpc, chain_id, tag) {
        Ok(true) => {
            let p = partial(share, &id);
            reply(
                200,
                json!({
                    "released": true,
                    "index": share.index,
                    "identity": hex::encode(id),
                    "partial": canon_hex(&p),
                }),
            )
        }
        Ok(false) => reply(
            200,
            json!({
                "released": false,
                "reason": "condition_not_met",
                "identity": hex::encode(id),
            }),
        ),
        Err(e) if e.is_transient() => reply(
            503,
            json!({"released": false, "reason": "chain_unavailable", "detail": e.to_string()}),
        ),
        Err(e) => reply(
            422,
            json!({"released": false, "reason": "rejected", "detail": e.to_string()}),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_std::rand::{rngs::StdRng, SeedableRng};
    use warden_core::shamir::split;

    fn a_share() -> Share {
        let mut rng = StdRng::seed_from_u64(1);
        let secret = ark_bls12_381::Fr::from(123u64);
        split(secret, 2, 3, &mut rng).unwrap()[0]
    }

    fn rpc() -> RpcClient {
        RpcClient::new("http://127.0.0.1:1")
    }

    #[test]
    fn malformed_body_is_400() {
        let r = handle_partial(
            "not json",
            &a_share(),
            &rpc(),
            84532,
            FinalityTag::Finalized,
        );
        assert_eq!(r.status, 400);
        assert!(r.body.contains("bad_request"));
    }

    #[test]
    fn unsupported_condition_is_422_without_touching_rpc() {
        // Wrong chain → rejected before any RPC read (rpc points at a dead port).
        let body = r#"{"condition":{"type":"contract","chain":1,"address":"0x00",
            "fn":"executed(uint256)","args":["1"],"test":{"cmp":"==","value":true},
            "meta":{"finality":32,"tier":1}}}"#;
        let r = handle_partial(body, &a_share(), &rpc(), 84532, FinalityTag::Finalized);
        assert_eq!(r.status, 422);
        assert!(r.body.contains("rejected"));
    }
}
