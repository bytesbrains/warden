# 01 — Architecture

## One-paragraph model

Warden is a federation of **operator nodes**, each holding a Shamir share of one **master key** produced by DKG. The master *public* key is published; the master *private* key is never assembled. A client encrypts a payload to an **identity = H(condition)** using only the master public key (no network interaction at encrypt time) — this is Boneh–Franklin IBE. When the condition becomes true on-chain, each node independently verifies it and emits a **partial decryption key** `sig_i = sk_i · H1(identity)`. The client gathers `t` partials, Lagrange-combines them into the decryption key `msk · H1(identity)`, and decrypts. (Math detail: [references/drand-analysis.md](references/drand-analysis.md) §5.)

## The double-wrap (why Warden never sees content)

A double-wrapped ciphertext nests two gates over one content key `K` (a Veil ciphertext is one example):

```
K         = random symmetric content key
inner     = AEAD(payload, K)                    ← the content
K_wrapped = ECIES(K, recipientPub)              ← recipient-gated. PURE MATH.
outer     = Warden-IBE(K_wrapped, H(condition)) ← condition-gated.
```

Warden's release only ever opens **outer** → it reveals `K_wrapped` (an ECIES ciphertext), **not** `K` and **not** the payload. Only the recipient's private key turns `K_wrapped` into `K`. So **both** the condition *and* the recipient key are required, and **even a fully-colluding federation learns only timing, never content.** See [04-envelope-format](04-envelope-format.md) and [05-threat-model](05-threat-model.md).

## Components

| Component | Role | Built on |
|---|---|---|
| **Node (`wardend`)** | Holds a key share; watches the condition's chain; serves partial decryptions | Rust core; drand-derived crypto |
| **DKG / dealer** | Establishes the shared master key (real DKG for mainnet; trusted-dealer for testnet) | drand Pedersen/Feldman DKG (reuse) |
| **Resharing** | Rotates shares to a new operator set **keeping the same master public key** | drand reshare (reuse) — the permanence mechanism |
| **Condition evaluator** | Each node independently reads chain state at a finalized block and decides release | **NEW** (the core Warden addition) |
| **Client SDK** | Encrypt to `H(condition)`; fetch+combine partials; decrypt | Rust core → TS (WASM) + Dart (FFI) |
| **Coordinator (optional)** | Helps clients discover nodes + collect partials | thin; client-orchestrated by default |

## Network planes (inherited from drand's hard-won design)

drand's v2.0 post-mortem (a reshare-downgrade attack from merging operator-control with node RPC) dictates strict plane separation — Warden adopts it:

- **Control plane** — local-only operator management (`wardenctl` → daemon over a localhost port). **Never internet-exposed.**
- **Private plane** — node↔node messaging for DKG/resharing (authenticated, behind TLS).
- **Public plane** — clients fetch partial decryptions (HTTP/relay). Read-only; partials are publicly verifiable against the master public key.

## Crypto: reuse vs replace (from the drand analysis)

**Reuse near-verbatim** (condition-agnostic):
- Threshold BLS on **BLS12-381** (Shamir shares, Lagrange combine).
- **Boneh–Franklin IBE / tlock** ciphertext `(U,V,W)` + Fujisaki–Okamoto CCA transform.
- **DKG** (Pedersen/Feldman) and **resharing** (old-share-as-free-coefficient ⇒ same master public key).
- **Unchained** semantics (identity predictable at encrypt time).
- The **age** hybrid-envelope pattern (a new `warden` stanza).

**Replace / add** (the genuinely new engineering):
- **Round scheduler → Base condition-watcher.** Nodes sign on *event*, not on a clock.
- **Identity = round number → identity = domain-separated `H(chainId ‖ contract ‖ beatId ‖ "executed")`.** Must be replay/grinding-proof and contract-bound (see [02-condition-model](02-condition-model.md)).
- **Finality policy** for the on-chain read (reorg-after-release = irreversible leak — pick confirmations conservatively).
- **On-demand, condition-keyed release** instead of a periodic randomness firehose.
- **Idempotent released-key store** — once a condition is met and released, served forever (mirrors "`executed` never reverts").

## Architecture decision (Phase 0.5 — DECIDED)

The Phase 0.5 spike ([08-architecture-decision](08-architecture-decision.md)) evaluated **DKG-IBE** vs **silent-setup + traitor-tracing** vs **SWE/McFly**. **Decision: build gen-1 on DKG threshold-IBE (the only audited, production-mature option)**; the challengers' assumptions are GGM/iO/trusted-CRS and their code is unaudited PoC — unsafe for seed-phrase-grade secrets today.

Key cross-cutting finding: **condition-gating is an app-layer policy in *all three*** (the Base-condition-watcher below is needed regardless). So we choose on **maturity + longevity**, and only DKG-IBE is mature.

**Roadmap (all swappable via the `alg` envelope):**
- **Gen-1 (now):** DKG threshold-IBE (tlock-derived, drand's audited stack).
- **Gen-2:** **SWE** — "no master secret" is longevity-optimal for a dead-man's switch; adopt when it has audited, cost-acceptable code.
- **Layer:** **threshold traitor-tracing** (ST3/2025-342) to close W1 (silent collusion), when production-ready.
- **Gen-3:** witness-encryption-from-SNARKs (trustless endgame).

## Language & multi-platform

Rust core (reasons: safety, mature crypto crates, and **one core → native node + WASM web client + Dart-FFI mobile client**, which closes the mobile gap that disqualified the vendors). Candidate crates: `blstrs`/`arkworks` (BLS12-381 + RFC-9380), `blsful` (threshold BLS, audited), `gennaro-dkg` (DKG, audited), `vsss-rs` (VSS), `ideal-lab5/timelock` (beacon-agnostic IBE timelock — closest architectural fit). All need a security audit before mainnet. Alternative: fork drand (Go) for the node and write Rust clients — faster to a hardened node, weaker mobile story. Decision: **Rust core** (see [07-roadmap](07-roadmap.md)).

## Data flow (worked example — Maktub Veil beat, happy path)

```
1. Owner (encrypt):   K=random; inner = AEAD(payload, K); K_wrapped = ECIES(K, recipientPub)
                      outer = IBE(K_wrapped, identity=H(condition=executed(beatId)))
                      publish {warden-v1 envelope} to Arweave/IPFS; CID on Base via MaktubCore
2. Owner alive:       checks in → executed stays false → no node releases → unreadable
   (or revokes:       deactivate → executed can never be true → never released → permanent gibberish)
3. Owner silent:      timer expires → any executor calls executeHeartbeat → executed(beatId)=true
4. Release:           client asks nodes for partials on H(condition);
                      each node verifies executed(beatId)==true at a finalized Base block → returns sig_i
                      client combines t partials → outer key → unwrap → inner ciphertext
5. Recipient (read):  ECIES-open K_wrapped → K  →  AEAD-open inner → plaintext
```
