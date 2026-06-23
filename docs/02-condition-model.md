# 02 — Condition Model

Warden is a **general** conditional-decryption network. `executed(beatId)==true` is one instance. This document specifies how conditions are expressed, bound to ciphertext, and evaluated.

## Identity = H(condition)

The IBE identity a payload is encrypted to is the hash of a **canonical, versioned condition spec**, serialized with **RFC 8785 (JSON Canonicalization Scheme, JCS)** so the bytes are identical across Rust / TypeScript / Dart implementations:

```
identity = H( "warden-cond-v1" ‖ jcs_rfc8785(condition) )
```

A node, asked for its partial on `identity`, is given the condition `C` (public metadata in the envelope), checks `H(C) == identity`, **independently evaluates `C` against finalized chain state**, and returns its partial **iff `C` holds**. This **binds the condition cryptographically to the ciphertext** — conditions cannot be swapped after encryption.

> The condition is **public** (it is metadata, not secret). For a Veil-style consumer it reveals `beatId` + that it gates on `executed` — already public on-chain. No new leakage.

## Condition schema (`cond-v1`)

**Shape** — exactly one `type`; every condition carries a `meta`:
- `contract` — `{ chain, address, fn, args, word?, test, meta }`
- `block` — `{ chain, field: "number"|"timestamp", cmp, value, meta }`
- `event` — `{ chain, address, sig, args, meta }`
- `all` | `any` | `not` | `threshold` — `{ of: [...sub-conditions], k (threshold only), meta }`

**Valid examples** (real JSON — examples must parse):

```json
{
  "type": "contract", "chain": 8453, "address": "0x…", "fn": "executed(uint256)",
  "args": ["12345678901234567890"], "test": { "cmp": "==", "value": true },
  "meta": { "finality": 32, "tier": 1 }
}
```
```json
{
  "type": "block", "chain": 8453, "field": "timestamp", "cmp": ">=", "value": 1893456000,
  "meta": { "finality": 32, "tier": 1 }
}
```

A worked consumer example — the release condition for a real Maktub Beat (Veil). `MaktubCore`
has **no `executed(uint256)` getter**; execution status is the 8th field (index 7) of
`getHeartbeat(uint256)`'s return tuple, so `word` selects it:

```json
{
  "type": "contract", "chain": 84532, "address": "0xb603C96D089F64Ac487EE0bdaE97D49848F86133",
  "fn": "getHeartbeat(uint256)", "args": ["123"], "word": 7,
  "test": { "cmp": "==", "value": true }, "meta": { "finality": 32, "tier": 1 }
}
```

**Type rules (load-bearing — the condition is hashed into the identity):**
- `cmp ∈ { "==", "!=", ">=", "<=", ">", "<" }` (string).
- **All `uint256` args/values are decimal *strings*** (e.g. `"12345…"`), never JSON numbers — to avoid JavaScript's `2^53−1` precision loss and guarantee byte-identical serialization across platforms.
- `word` (contract only, optional, default `0`) — index of the 32-byte ABI return word to compare. `0` for a single-value getter; for a tuple getter, the target field's ordinal position (static fields sit inline in the ABI head regardless of earlier dynamic types). **Omitted from the canonical form when `0`**, so single-value conditions hash identically to the pre-`word` schema.
- Serialization is **RFC 8785 (JCS)**: sorted keys, no insignificant whitespace, fixed number/string formatting. A type mismatch (number vs string) yields a *different* `identity` and **silently breaks decryption**.

## The three non-negotiable constraints

A threshold network only works if every honest node reaches the *same* yes/no. Therefore:

1. **Determinism.** Conditions are evaluated against **finalized** chain state at the confirmation depth in `meta.finality`. No live/unfinalized reads.
2. **Monotonicity (latching).** Prefer conditions that, once true, **stay true** (e.g. `executed==true`). The decryption key, once released, **cannot be un-released**, and retries must always succeed. Non-monotonic raw predicates (e.g. `price > X`) are **rejected** unless **anchored** to make them permanent — e.g. `"became true at/by block B"` or an `event`-was-emitted form.
3. **Finality / reorg safety.** Release only on finalized state; a reorg that reverts the condition *after* release is an **irreversible confidentiality leak**. Choose `finality` conservatively (see [05-threat-model](05-threat-model.md)).

## Identity domain-separation (security-critical)

Unlike drand round numbers (globally unique by construction), condition-derived identities are attacker-influenceable. To prevent **cross-item key reuse** or **grinding**, `H1` (hash-to-curve of the identity) **must** be strongly domain-separated and bind the **chain id + contract address + full condition**. A partial minted for one condition must never open a ciphertext for another. (drand-analysis §9.4.)

## Revocability is the app's responsibility

Warden only *evaluates* conditions. **Revocation** = the app constructs a condition that can be made **permanently unsatisfiable**. Maktub's Veil does this: `deactivate(beatId)` means `executed(beatId)` can never become true → Warden never releases → permanent gibberish. Apps wanting revocability must design conditions with this property.

## Trust tiers

- **Tier 1 — on-chain state** (`contract` / `block` / `event`, finalized): deterministic, strong. **Ship this for v1** (and the Veil consumer).
- **Tier 2 — external data** (oracle values, JSON/REST conditions): adds a **data-source trust + determinism risk**. **Opt-in, later phase.** Operators *declare* which tiers they evaluate; clients see the tier and decide.

## Use cases unlocked

| Condition | Use case |
|---|---|
| `executed(beatId)==true` | **Maktub Veil** — dead-man's switch *(reference example)* |
| `block.timestamp >= T` | **Time capsule** — open on a future date |
| `any(executed==true, timestamp>=deadline)` | **Multi-trigger** — deliver on silence *or* a hard deadline |
| `all(executed==true, timestamp>=minDate)` | **Min-date guard** — never before a floor date |
| `Project.milestoneReached()==true` | **Milestone escrow** |
| `Governor.proposalPassed(id)==true` | **DAO** — reveal on a passed vote |
| `<anyEVMcontract>.<getter>` on any chain | **Any third-party app** — the adoption story |

## Phasing

- **v1 (MVP, e.g. the Veil consumer):** Tier-1 `contract` + `block` + `all/any/not`.
- **v1.x:** cross-chain hardening, `event` type, `threshold` (k-of-N) compound.
- **v2:** Tier-2 (oracle/API), opt-in, separately tiered.
