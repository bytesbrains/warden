# 08 — Architecture Decision (Phase 0.5 spike)

**Status:** DECIDED · **Date:** 2026-06-17 · Supersedes the "DKG under review" note in [01-architecture](01-architecture.md).
Evidence: deep-reads of STE+tracing and SWE/McFly (full notes summarized below); baseline from [references/drand-analysis.md](references/drand-analysis.md) and [references/crypto-scan-2026-06.md](references/crypto-scan-2026-06.md).

## The decision

**Gen-1 (build now): DKG threshold-IBE (tlock-derived).** Reuse drand's *audited, production* threshold-BLS + Boneh–Franklin IBE + DKG + resharing. It is the **only** option mature enough for seed-phrase-grade, decades-sealed secrets today.

- **Layer in threshold traitor-tracing** (ST3 / 2025/342) as a roadmap item to close weakness **W1** (undetectable collusion) — *when it has audited code*. Trackable, not blocking.
- **Target SWE (McFly family) for Gen-2** — its "no master secret" property is the longevity-optimal design for a dead-man's switch; adopt when it has audited, production-grade, cost-acceptable implementations.
- **Target witness-encryption-from-SNARKs for Gen-3** — the trustless endgame.
- All swappable via the `alg`-versioned envelope ([04-envelope-format](04-envelope-format.md)) — **no consumer change** between generations.

## The cross-cutting finding that drove it

**Condition-gating is an application-layer policy in *all three* architectures.** None of them cryptographically enforces "release only when `executed(beatId)==true`":
- DKG-IBE and STE both bind decryption to "*t* parties cooperate" — the nodes voluntarily watch Base and release only when the condition holds.
- SWE binds to "*k* parties signed the tag" — the federation must sign `(beatId, executed=true)` when it observes it.

So the choice between architectures is **not about the gate** (the Base-condition-watcher + finality policy is needed regardless — see [03-protocol](03-protocol.md) §5). It's about **key-custody, longevity, collusion-detectability, and maturity.**

## Scoring matrix

| Dimension | **DKG-IBE (tlock)** ← gen-1 | STE + tracing | SWE / McFly |
|---|---|---|---|
| **Operator complexity** | 🟡 needs DKG + resharing | 🟢 no DKG (silent setup) | 🟢 no DKG, no master secret |
| **Decades-horizon survival** | 🟡 master secret persists; resharing keeps key; but one `msk` break = total loss | 🔴 static shares, **no repair**, CRS caps universe N, **not PQ** | 🟡 **no master secret** (win); but original committee keys must stay signable; collusion blast = *one beat* |
| **Collusion-detectability** | 🔴 none | 🟢 **public traitor-tracing** (the prize) | 🟡 per-tag blast radius; explicit tracing unconfirmed |
| **Condition-gating fit** | 🟢 app-layer (works) | 🟢 app-layer (STE adds nothing here) | 🟢 sign the condition-tag (eWEB can't pre-seal → unusable for us) |
| **Mobile fit** | 🟢 cheap, `tlock-rs` (wasm/aarch64) | 🟢 public combine, tiny ct | 🟡 **O(n) ciphertext + decrypt** (14.8 s @ n=500 laptop) → needs small committee |
| **Audit / deployable today** | 🟢 **audited (Kudelski), live since 2023** | 🔴 unaudited PoC; **GGM** + **trusted CRS** | 🔴 **dormant toy prototypes**; compact variant = **iO** (pure theory) |
| **Trust assumptions** | 🟢 standard pairing + ROM, audited | 🔴 GGM + structured CRS (trusted setup) | 🟡 pairing + honest-majority (compact = iO) |

## Why not the challengers *now*

- **STE doesn't deliver what the scan hoped.** It removes DKG, but: (a) it does **not** improve decades-horizon survival (static shares, no repair, CRS universe cap, no PQ); (b) it trades DKG for a **trusted CRS + GGM** idealization; (c) the encryption variant is **unaudited PoC**. Its *one* genuine prize — **traitor-tracing** — is a *layer* we can adopt later regardless of the base scheme.
- **SWE is the better long-term *design*, but the worst *maturity*.** "No master secret, oblivious signers, per-beat blast radius" is exactly right for a dead-man's switch — but the only code is **dormant 2★ solo prototypes**, ciphertext is **linear in committee size** (heavy on mobile), and the "compact" version needs **iO** (unbuildable). `eWEB`/2025/1064 does **not** let the chain be the witness on its own (still an honest-majority committee, next-block-only, can't pre-seal to "whenever `executed` flips").
- **Building mainnet on unaudited research crypto for seed-phrase secrets is irresponsible.** DKG is "the hard part," but it is a **solved, audited** hard part (drand) — we *reuse* it, not reinvent it.

## Consequences

- **Phase 0/1 unchanged:** testnet uses **trusted-dealer shares** (no DKG anyway); mainnet uses drand's audited DKG + resharing (the permanence mechanism).
- **The Base-condition-watcher + finality policy is built regardless** — it's architecture-independent and remains the genuinely novel, security-critical component.
- **W1 (silent collusion) stays an accepted-but-bounded risk for gen-1**, with traitor-tracing as the tracked mitigation (not yet production).
- **Envelope `alg`-versioning is now a hard requirement** so gen-2 (SWE) / gen-3 (WE) swap in without touching Maktub or recipients.

## Open items to verify from full PDFs (ePrint 403-blocked automated fetch)
1. **STE old-ciphertext survival across operator *departure*** (2024/263) — does the helper-key mechanism keep a years-old ciphertext decryptable when original quorum members leave?
2. **Tracing model** (2025/342, ST3) — public vs secret traceability; exact assumptions; concrete ciphertext blowup at our scale.
3. **SWE longevity** (2022/433) — confirm *only* signer key-availability must persist (no shared secret); small-committee (n≈10–50) decrypt benchmarks; named hardness assumption.
4. **Re-run this spike semi-annually** — SWE maturing to audited production code, or WE-from-SNARKs becoming practical, flips the gen-2/gen-3 timing.

## One-line summary

> **Build gen-1 on the mature, audited DKG threshold-IBE (tlock); add traitor-tracing when it's production-ready; migrate to SWE (no-master-secret) and then witness-encryption as they mature — all via the `alg` envelope. The condition-gate is app-layer in every option, so we choose on maturity and longevity, and only the mature option is safe to ship for real secrets.**
