//! A tiny read-only Ethereum JSON-RPC client (blocking, over `ureq`).
//!
//! Only what the condition-watcher needs: `eth_call` and `eth_getBlockByNumber`, always at a
//! fixed block tag (`finalized` by default — see [`crate::config::FinalityTag`]). Read-only:
//! the node never sends transactions and holds no chain key.

use serde_json::{json, Value};

/// Errors talking to the RPC endpoint. All are treated by the handler as **transient**
/// (the client should retry) — a node that can't read the chain must never release.
#[derive(thiserror::Error, Debug)]
pub enum RpcError {
    #[error("transport: {0}")]
    Transport(String),
    #[error("rpc error: {0}")]
    Rpc(String),
    #[error("unexpected rpc response shape")]
    BadResponse,
    #[error("hex decode of rpc result: {0}")]
    Hex(String),
}

pub struct RpcClient {
    url: String,
    agent: ureq::Agent,
}

impl RpcClient {
    pub fn new(url: &str) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(15))
            .build();
        RpcClient {
            url: url.to_string(),
            agent,
        }
    }

    fn call(&self, method: &str, params: Value) -> Result<Value, RpcError> {
        let req = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
        let resp: Value = self
            .agent
            .post(&self.url)
            .send_json(req)
            .map_err(|e| RpcError::Transport(e.to_string()))?
            .into_json()
            .map_err(|e| RpcError::Transport(e.to_string()))?;
        if let Some(err) = resp.get("error") {
            return Err(RpcError::Rpc(err.to_string()));
        }
        resp.get("result").cloned().ok_or(RpcError::BadResponse)
    }

    /// `eth_call` to `to` with `data`, evaluated at `block_tag`. Returns the raw return bytes.
    pub fn eth_call(&self, to: &str, data: &[u8], block_tag: &str) -> Result<Vec<u8>, RpcError> {
        let params = json!([
            { "to": to, "data": format!("0x{}", hex::encode(data)) },
            block_tag
        ]);
        let result = self.call("eth_call", params)?;
        let s = result.as_str().ok_or(RpcError::BadResponse)?;
        decode_hex(s)
    }

    /// A `u64` field (`"number"` or `"timestamp"`) of the block at `block_tag`.
    pub fn block_field(&self, field: &str, block_tag: &str) -> Result<u64, RpcError> {
        let params = json!([block_tag, false]);
        let block = self.call("eth_getBlockByNumber", params)?;
        if block.is_null() {
            // No finalized block yet (fresh chain). Treat as "cannot determine" → transient.
            return Err(RpcError::Rpc(format!("no block at tag {block_tag}")));
        }
        let hexstr = block
            .get(field)
            .and_then(|v| v.as_str())
            .ok_or(RpcError::BadResponse)?;
        let bytes = decode_hex(hexstr)?;
        // Field fits in u64 for any realistic block number / timestamp. Use checked mul/add
        // (not `checked_shl(8)`, which only fails for shifts >= 64) so a malicious RPC
        // returning a >u64 value errors out instead of silently wrapping.
        let mut acc: u64 = 0;
        for b in bytes {
            acc = acc
                .checked_mul(256)
                .ok_or(RpcError::BadResponse)?
                .checked_add(b as u64)
                .ok_or(RpcError::BadResponse)?;
        }
        Ok(acc)
    }
}

/// Decode a `0x`-prefixed hex string (odd-length nibble strings are left-padded).
fn decode_hex(s: &str) -> Result<Vec<u8>, RpcError> {
    let body = s.strip_prefix("0x").unwrap_or(s);
    let padded;
    let body = if body.len() % 2 == 1 {
        padded = format!("0{body}");
        padded.as_str()
    } else {
        body
    };
    hex::decode(body).map_err(|e| RpcError::Hex(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_hex_handles_prefix_and_odd_length() {
        assert_eq!(decode_hex("0x01").unwrap(), vec![1]);
        assert_eq!(decode_hex("0x1").unwrap(), vec![1]); // odd nibble count
        assert_eq!(decode_hex("0x").unwrap(), Vec::<u8>::new());
        assert!(decode_hex("0xzz").is_err());
    }
}
