//! Node configuration — loaded from environment variables (12-factor; fits docker-compose).
//!
//! | Var | Required | Default | Meaning |
//! |---|---|---|---|
//! | `WARDEN_SHARE_FILE` | yes | — | Path to this node's `node-<i>.json` (its secret share). |
//! | `WARDEN_RPC_URL` | yes | — | Base Sepolia JSON-RPC endpoint (read-only). |
//! | `WARDEN_LISTEN` | no | `0.0.0.0:8080` | HTTP bind address for the partial-serving endpoint. |
//! | `WARDEN_CHAIN_ID` | no | `84532` | The chain id this node watches; conditions for other chains are refused. |
//! | `WARDEN_FINALITY_TAG` | no | `finalized` | Block tag conditions are evaluated at. |

use std::env;

use warden_core::fed::NodeShareFile;
use warden_core::ibe::MasterPublicKey;
use warden_core::shamir::{Share, ShareIndex};

/// Base Sepolia. The PoC condition source (issue #181 / D-036).
pub const DEFAULT_CHAIN_ID: u64 = 84532;
const DEFAULT_LISTEN: &str = "0.0.0.0:8080";
const DEFAULT_FINALITY_TAG: &str = "finalized";

/// Block tag at which on-chain conditions are read. `finalized` is the conservative,
/// reorg-safe choice (Base: L1-finalized) and the intended federation-wide floor; `safe` /
/// `latest` exist only for local testing against a fresh chain and are **not** reorg-safe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinalityTag {
    Finalized,
    Safe,
    Latest,
}

impl FinalityTag {
    pub fn as_rpc(&self) -> &'static str {
        match self {
            FinalityTag::Finalized => "finalized",
            FinalityTag::Safe => "safe",
            FinalityTag::Latest => "latest",
        }
    }
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "finalized" => Ok(FinalityTag::Finalized),
            "safe" => Ok(FinalityTag::Safe),
            "latest" => Ok(FinalityTag::Latest),
            other => Err(format!(
                "WARDEN_FINALITY_TAG must be finalized|safe|latest, got {other:?}"
            )),
        }
    }
}

/// Resolved node configuration + loaded share material.
pub struct Config {
    pub index: ShareIndex,
    pub network: String,
    pub share: Share,
    pub mpk: MasterPublicKey,
    pub rpc_url: String,
    pub listen: String,
    pub chain_id: u64,
    pub finality: FinalityTag,
}

fn require(var: &str) -> Result<String, String> {
    env::var(var).map_err(|_| format!("missing required env var {var}"))
}

impl Config {
    /// Build from the process environment, loading + validating the share file.
    pub fn from_env() -> Result<Self, String> {
        let share_path = require("WARDEN_SHARE_FILE")?;
        let rpc_url = require("WARDEN_RPC_URL")?;
        let listen = env::var("WARDEN_LISTEN").unwrap_or_else(|_| DEFAULT_LISTEN.to_string());
        let chain_id = match env::var("WARDEN_CHAIN_ID") {
            Ok(s) => s
                .parse()
                .map_err(|_| format!("WARDEN_CHAIN_ID must be an integer, got {s:?}"))?,
            Err(_) => DEFAULT_CHAIN_ID,
        };
        let finality = FinalityTag::parse(
            &env::var("WARDEN_FINALITY_TAG").unwrap_or_else(|_| DEFAULT_FINALITY_TAG.to_string()),
        )?;

        let raw = std::fs::read_to_string(&share_path)
            .map_err(|e| format!("reading share file {share_path}: {e}"))?;
        let nf: NodeShareFile = serde_json::from_str(&raw)
            .map_err(|e| format!("parsing share file {share_path}: {e}"))?;
        // `share()` validates 1<=t<=n and that the embedded index matches the declared one.
        let share = nf.share().map_err(|e| format!("invalid share: {e}"))?;
        let mpk = nf
            .master_public_key()
            .map_err(|e| format!("invalid master pubkey in share file: {e}"))?;

        Ok(Config {
            index: nf.index,
            network: nf.network,
            share,
            mpk,
            rpc_url,
            listen,
            chain_id,
            finality,
        })
    }
}
