//! # warden-core (Phase 0 PoC)
//!
//! Core cryptography for **Warden** — the event-gated threshold conditional-decryption
//! network that powers Veil. See `warden/docs/` for the full specification.
//!
//! Modules (Phase 0 PoC plan, Maktub issue #181, WS-A):
//! - [`condition`] — the condition model, RFC-8785-style canonicalization, and the
//!   domain-separated `identity = H(condition)` a payload is IBE-encrypted to.
//! - [`shamir`] — Shamir secret sharing over BLS12-381 `Fr` + Lagrange at `x=0`.
//! - [`ibe`] — Boneh–Franklin IBE over BLS12-381 (tlock-style) with threshold
//!   partial-decryption, partial **verification**, and combine.
//! - [`dealer`] — trusted-dealer setup (**`trusted-dealer` feature**, testnet only;
//!   real DKG replaces it for mainnet).
//! - [`envelope`] — the `warden-v1` double-wrap (condition gate + recipient gate).
//! - [`fed`] — the federation file format (public master key + share pubkeys; per-node
//!   secret share files) that the dealer emits and nodes/clients load.
//!
//! ⚠️ **Not audited. Not for production.** All-ours testnet = zero security by design.

#![forbid(unsafe_code)]

pub mod condition;
#[cfg(feature = "trusted-dealer")]
pub mod dealer;
pub mod ecies;
pub mod envelope;
pub mod fed;
pub mod ibe;
pub mod shamir;

pub use condition::{Condition, Meta, Test};
