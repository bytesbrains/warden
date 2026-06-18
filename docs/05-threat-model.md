# 05 — Threat Model

State exactly what Warden does and does not protect — over- and under-claiming are both failures (Maktub honesty guardrail D-031).

## The two-axis trust split

| Property | Rests on | Strength |
|---|---|---|
| **Content confidentiality** (only the recipient ever reads plaintext) | the recipient's own key (ECIES **inner** layer) | **Pure math.** Even a fully-colluding Warden cannot read content. |
| **Timing + revocation** (unreadable until the condition holds) | the federation's **honest majority** *and* nodes **correctly observing the on-chain condition** | **Honest-majority**, not trustless. |

The double-wrap is what confines the federation's power to *timing*. A colluding federation can release the *outer* key early, or refuse it — but never read content.

## Weaknesses (exhaustive for the deployable design)

### W1 — Early reveal (bribe a threshold)
A receiver who bribes/compromises `t` operators can get the outer key released **before** the condition holds, and — with their own key — read early. Bounds:
- Only the **receiver** benefits (content is inner-locked); a third-party briber gains nothing readable.
- Cost = corrupt `t` independent operators. Defense is **economic + diversity** (large, jurisdictionally-diverse federation; higher `t`).
- Where it bites: **high-value beats** (large-wallet inheritance) where the bribe-cost bar = the wallet's value.
- **Mitigation (optional):** trigger-gated identity (Veil "Layer 0′") forces such an attack to be a **loud, detectable blanket release** rather than a quiet targeted one.
- **Update (2026-06 scan):** **threshold traitor-tracing** (ST3 / 2025/342) can make a colluding committee **publicly attributable** — turning W1 from *undetectable* to *detectable+deterrable*. Previously we treated silent collusion as unfixable; it may not be. Under evaluation in [Phase 0.5](07-roadmap.md). See [references/crypto-scan-2026-06.md](references/crypto-scan-2026-06.md).

### W2 — Liveness / denial → mostly DELAY, not permanent loss
Because the ciphertext and the condition are **permanent**, release is **retryable forever**. A temporarily-down/refusing network causes **delayed** delivery, not loss. The receiver can pull anytime.
- **Permanent loss** only if **all** redundant Warden generations/networks die forever → mitigated by resharing (permanence-with-churn) + multi-network redundancy. (No first-party backup — see W4.)
- Delay severity is use-case-dependent: fine for inheritance; harmful for urgent safety triggers → argue for high availability there.

### W3 — Finality / reorg leak (new, no drand precedent)
If a node releases on an `executed==true` state that is then **reorged away**, the reveal is **irreversible**. Mitigation: release only on **finalized** state at a conservative confirmation depth ([03-protocol](03-protocol.md) §5). This is a genuinely new, security-critical surface Warden adds over drand.

### W4 — Operator/infra capture & the no-first-party-key rule
- A federation entirely controlled by one party (e.g. all nodes in one cloud account) collapses to **1 party** — compellable + capable of early release. **Warden never holds a release key itself**, and the federation must be **independent operators** for any real security. Testnet (all-ours) has **zero** security by design.
- Inherit drand's **v2.0 lesson**: strict control-plane/node-plane separation + `OldThreshold` downgrade protection on every reshare, or a malicious operator can chain reshares to reduce the threshold to themselves and recover the master key.

### W5 — Master-key permanence is double-edged
The master public key is immutable across reshares (the permanence feature). Consequence: **any** eventual reconstruction of `msk` breaks **all** past and future ciphertexts under that key — and ciphertexts may sit dormant for years. Reshare hygiene is non-negotiable; consider a key-rotation/new-generation migration story (opt-in re-encryption, mirroring Maktub's immutable-V2 pattern).

### W6 — Not quantum-resistant
BLS/IBE are pairing-based, not PQ-secure (stated tlock horizon ~5 years). For "survives you by decades" payloads this is a real limit to disclose to product/legal. Track a PQ migration path.

### W7 — Condition-evaluation trust (Tier-2)
Tier-1 (on-chain, finalized) is deterministic. **Tier-2** (oracle/API data) adds a **data-source trust** and determinism risk; keep it opt-in and clearly labeled. Identity must be domain-separated so a partial for one condition cannot open another (grinding/replay).

### W8 — Open-request metadata channel (client → node)
Opening a Veil envelope POSTs the **condition** (which embeds `beatId` + core address + chainId) to every node's `/partial`. So each node operator — and any on-path observer of that request — learns **requester-IP ↔ beatId interest** and the *timing* of the open attempt. The `beatId` and condition are already public on-chain; what's new is binding them to a requesting IP at open time. This is consistent with "not metadata-private" below, not a content leak (a tampered condition simply never releases — fail-closed). Mitigations for the independent-operator federation: client egress over a privacy-preserving transport (e.g. Tor/oblivious relay), and TLS pinning / a known-key channel to deny passive collectors the linkage. **Preview note:** with the all-ours testnet federation this is moot, but it MUST be tracked before independent operators. (App side: `mobile/lib/services/crypto/veil/veil.dart` `_fetchPartial`; tracked in #237.)

## What Warden is NOT trying to be

- **Not trustless.** The trustless ideal is **witness encryption** (the on-chain proof *is* the key, no federation) — not deployable today. Warden is the best deployable approximation and is designed to migrate to WE via the `alg` envelope when it ships.
- **Not metadata-private.** Existence, timing, condition, and (for Veil) `beatId` are public. Warden hides *content until the trigger*, never *that something exists*.

## Honest claim language (for any copy)

- **MAY say:** time-bound, revocable, end-to-end-sealed delivery enforced by an independent threshold federation's honest majority and liveness, with redundancy.
- **MUST NOT say:** "absolute," "mathematically guaranteed" (for timing/revocation), "bulletproof," "absolute anonymity," "prevents bribery," "cannot leak."
