# Recent-cryptography scan (2024–2026) — implications for Warden

> **Scanned 2026-06-17** (arXiv cs.CR + IACR ePrint). Verdict: **not "proceed unchanged."** Two *deployable-today* advances attack Warden's stated worst risks (DKG complexity; undetectable collusion), and one (SWE/McFly) reframes the release layer toward the trustless ideal. **Before building DKG (the hard part), run an architecture spike** comparing the options — see [`../07-roadmap.md`](../07-roadmap.md) Phase 0.5.
>
> Caveat: ePrint blocks automated PDF fetch, so findings are **abstract/proceedings-level**. Pull full PDFs of 2022/433, 2024/1477, 2025/1064, 2024/263, 2025/342 before committing.

## Top 3 findings that warrant a redesign / different perspective

### 1. Signature-Based Witness Encryption (SWE) — *perspective shift on the whole release layer*
- McFly (2022/433) → compact-ciphertext SWE (2024/1477) → blockchain/next-block SWE "eWEB" (2025/1064).
- **Idea:** encrypt against a committee's verification keys + a **tag**; **the committee's (threshold) signature on the tag IS the decryption witness.** Users never talk to the committee; no per-item decryption key is ever held.
- **Why it matters for Warden:** our current design has a DKG federation holding **per-item IBE decryption shares** and a **master secret that must survive decades** (the W5 risk). SWE collapses this to *"encrypt to whoever signs `(beatId, executed=true)`; their signature decrypts."* It **removes per-item key custody, removes the user↔committee interaction, and reshapes the master-secret-longevity risk.** Uses the **same BLS12-381/threshold-BLS substrate** we already planned. eWEB (2025/1064) is literally "secret contingent on next-block state" — the shape of our condition.
- **Impact: REDESIGN candidate — prototype it.** This is the closest *deployable* approximation of our trustless witness-encryption ideal.

### 2. Silent Threshold Encryption (no DKG) + Threshold Traitor-Tracing — *kills our two worst liabilities*
- Threshold Encryption with **Silent Setup** (2024/263): the joint public key is a **deterministic function of operators' locally-published keys — no DKG, no interactive ceremony.** Concretely efficient, implemented.
- **Silent Threshold Traitor-Tracing (ST3)** (CCS 2025; no-dealer tracing 2025/342): silent setup **plus public tracing of a colluding committee, no trusted authority**, from pairings.
- **Why it matters for Warden:**
  - **Deletes DKG** — the single biggest operational/security liability and audit surface (and the source of drand's v2.0 reshare-downgrade attack class). Operators just locally publish keys.
  - **Closes weakness W1** — Warden currently **cannot detect silent early-reveal collusion**; threshold traitor-tracing makes it **publicly attributable**, adding real deterrence to the bribe attack.
- **Impact: ADOPT (evaluate seriously).** If we keep a committee (vs going full SWE), this is the design.

### 3. Crypto-agility for the post-quantum horizon — *no redesign, but make it a hard requirement*
- PQ time-lock puzzles from lattices (2025/047), PQ-IBE from ML-KEM/ML-DSA (2025/2143), lattice RBE (2026/628). All **theoretical / not deployable at threshold-IBE scale.**
- **State of the art:** there is **no deployable PQ replacement for threshold-IBE/BLS** today. Our decades-horizon claim is **unbacked at the threshold layer** — say so internally.
- **Impact: WATCH + mandate.** Keep the envelope `alg`-versioning a **hard requirement** (already in [`../04-envelope-format.md`](../04-envelope-format.md)); PQ-harden the **inner recipient layer first** (deployable PQ KEMs exist), since the outer layer has no PQ option.

## Lower-impact findings

- **Witness Encryption from SNARKs / KZG** (2025/1364 CRYPTO'25; 2024/264) — full *trustless* WE for raw Base state is **closer but still theoretical** (needs Base's `executed` flag expressed as a SNARK/KZG statement; idealized assumptions). **WATCH** — this is the strand that would eventually obsolete *any* committee. Track 2025/1364.
- **Registration-Based Encryption** (2025/502; 2026/628) — removes *recipient-key* escrow, which our **inner ECIES layer already handles**. Does **not** give `identity = H(condition)`. **NO CHANGE.**
- **DyCAPS / async proactive secret sharing** (2025) — cleaner *resharing* engine if we keep a DKG federation, but incremental, and still assumes a known bounded committee. **WATCH.** Decide DKG-vs-silent-setup first; the resharing/permanence story follows from that.

## What this changes in our plan

1. **Insert an architecture spike before Phase 0** (see roadmap Phase 0.5): compare **(i) DKG-federation (current)** vs **(ii) silent-setup threshold + traitor-tracing** vs **(iii) SWE/McFly against a signing committee** — scored on *operator complexity, decades-horizon key survival, collusion-detectability, mobile-client fit, audit surface*.
2. **Likely direction (pending full-paper read):** drop DKG in favor of **silent setup**, add **traitor-tracing**, and seriously evaluate **SWE** as the release model — all on the BLS12-381 substrate already chosen. This would *simplify* the build and *strengthen* the threat model simultaneously.
3. **Keep witness-encryption-from-SNARKs and PQ on a semi-annual WATCH;** keep `alg`-versioning a hard requirement so any of these swap in without consumer changes.

## Sources
WE/SNARK: 2025/1364, 2024/264, 2022/1510 · Silent setup: 2024/263 · Traitor-tracing: 2025/342, ST3 (CCS 2025, dl.acm 10.1145/3719027.3765099), 2025/1347, 2025/2154 · SWE: 2022/433, 2024/1477, 2025/1064 · RBE: 2025/502, 2026/628 · DyCAPS: cje.2025.00.072 · PQ: 2025/047, 2025/2143 · tlock baseline: 2023/189.
