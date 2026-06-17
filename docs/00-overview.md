# 00 — Overview

## What Warden is

Warden is an **event-gated threshold conditional-decryption network**. A set of independent operator nodes jointly hold one master key (no node ever holds the whole key — see DKG in [03-protocol](03-protocol.md)). A client encrypts a payload "to a condition." When that **on-chain condition becomes true**, a threshold of nodes each release a *partial* decryption key; the client combines them to recover the decryption key and read the payload. Before the condition holds, the payload is unreadable by **everyone — including the intended recipient**.

This is the same cryptography as drand's **tlock** (Boneh–Franklin IBE over threshold BLS), with one change: the release trigger is **an on-chain condition** (`executed(beatId)==true`) instead of **time** (round number reached). See [references/drand-analysis.md](references/drand-analysis.md) §8.

## Why it exists

It powers **Veil**, Maktub's time-bound delivery. But the diligence that killed every third-party network (Maktub issue #179) also revealed that the long-horizon, owner-absent dead-man's-switch use case needs a key-release network that:
- gates on a **resettable on-chain condition** (no vendor ships this productized),
- commits to **permanence** (immutable master key; no forced re-keying / sunsets), and
- has a **native multi-platform client** (incl. Flutter mobile).

No existing network meets all three. Warden is built to.

## Goals

1. **Time-bound conditional decryption** — unreadable until an on-chain condition holds.
2. **Revocability** — an app can construct a condition that is made *permanently unsatisfiable* (Maktub does this via `deactivate`), making the ciphertext permanent gibberish.
3. **Permanence with churn** — the master public key survives operator turnover via **resharing**, so ciphertexts sealed today decrypt years later even as the federation changes.
4. **General conditions** — `executed==true` is one instance; support contract-state / time / event / boolean-compound / cross-chain conditions (see [02-condition-model](02-condition-model.md)) so other apps adopt it.
5. **No token, minimal gas** — operates off-chain, reads chains for free; federation runs as a public good (drand/League-of-Entropy model).
6. **Multi-platform clients** — one Rust core → node + WASM (web/TS) + FFI (Flutter/Dart).

## Non-goals

- **Not** a blockchain, **not** a token, **not** consensus over arbitrary state.
- **Not** a storage layer (payloads live on Arweave/Filecoin/IPFS; Warden only handles keys).
- **Not** a custodian of plaintext — content stays ECIES-encrypted to the recipient (double-wrap); Warden gates *timing*, never *content*.
- **No** governance over its own behavior beyond operator membership; **no** trusted single party.

## Relationship to Veil and Maktub

- **Veil** = the capability *inside* Maktub (the user-facing property: sealed until you go silent, revocable, E2E).
- **Warden** = the network that *enforces* the timing/revocation half of Veil.
- **Maktub Beat** = the immutable on-chain contract that provides the condition (`executed`). Warden requires **no change to `MaktubCore`** — it only *reads* `executed(beatId)`.

## The honest trust summary (see [05-threat-model](05-threat-model.md))

- **Content confidentiality** rests on the recipient's key (ECIES inner layer) — **pure math**; even a fully-colluding Warden cannot read content.
- **Timing + revocation** rest on the federation's **honest majority** *and* on nodes **correctly observing the on-chain condition** — strong, but not trustless. The trustless ideal (witness encryption) is not deployable; Warden is the best deployable approximation, designed to be honest about exactly what it does and does not guarantee.
