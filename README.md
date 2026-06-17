# Warden

**Warden** is a drand-derived, **event-gated threshold conditional-decryption network**: a federation of independent nodes that hold shares of one master key (via DKG) and release a per-item decryption key **only when an on-chain condition becomes true** (e.g. `MaktubCore.executed(beatId) == true` on Base).

It is the network that powers **Veil** — Maktub's time-bound, revocable, end-to-end-sealed delivery — and is designed as a **standalone public good** that any application needing "decrypt only when this on-chain condition holds" can build on.

> *"Veil seals the letter; Warden keeps the key until the moment, then releases it."*

## Status

**Pre-alpha / private development.** Specs first, then a PoC, then a public testnet, then open-source + audit, then a mainnet federation. See [`docs/07-roadmap.md`](docs/07-roadmap.md). **Not yet built; do not use for real secrets.**

## What it is / is not

- **Is:** an off-chain threshold-IBE key-release network. Reuses drand's cryptography (threshold BLS on BLS12-381, Boneh–Franklin IBE / tlock, DKG, resharing); replaces drand's *time/round* trigger with an *on-chain-condition* trigger.
- **Is not:** a blockchain, a token, a storage layer, or a custodian of plaintext. Warden never sees plaintext (content stays end-to-end encrypted to the recipient — see the double-wrap in [`docs/01-architecture.md`](docs/01-architecture.md)). Warden gates *timing*, not *content*.

## Repository layout

| Path | Purpose |
|---|---|
| `docs/00-overview.md` | What Warden is, goals, non-goals |
| `docs/01-architecture.md` | System architecture; drand reuse/replace |
| `docs/02-condition-model.md` | The general condition spec (identity = `H(condition)`) |
| `docs/03-protocol.md` | DKG, threshold release, resharing, finality |
| `docs/04-envelope-format.md` | The ciphertext envelope (`warden-v1`) |
| `docs/05-threat-model.md` | Trust model, weaknesses, tiers |
| `docs/06-operator-manual.md` | Node requirements + partner onboarding |
| `docs/07-roadmap.md` | Phased plan (PoC → testnet → open → mainnet) |
| `docs/GLOSSARY.md` | Terms (DKG, IBE, threshold BLS, …) |
| `docs/references/drand-analysis.md` | Fetched + analyzed drand v2 reference |

## Relationship to Maktub

Tracked in the Maktub repo under the `warden` label: decision/exploration **#177**, Veil spec **#178**, key-network diligence **#179**. License target: **MIT** (consistent with Maktub protocol/SDK).
