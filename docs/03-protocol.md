# 03 — Protocol

Covers: key setup (DKG / dealer), the release flow, resharing, finality, and the on-demand request protocol. Crypto detail and drand provenance: [references/drand-analysis.md](references/drand-analysis.md).

## 1. Key setup

### Mainnet: real DKG (no dealer)
All operators jointly run **one** DKG ceremony (Pedersen/Feldman VSS — drand-derived). Output: one **master public key** + one **share per operator**; the master *private* key is never assembled. Threshold `t` of `n` (target BFT-style `t ≈ ⌈2n/3⌉`; minimum `⌊n/2⌋+1`). Inherit drand's lifecycle (`joiners`/`remainers`/`leavers`) and **strict control-plane/node-plane separation** + **`OldThreshold` downgrade protection** (the drand v2.0 attack class).

### Testnet: trusted dealer (shortcut)
A one-off script generates the master key, Shamir-splits it into `n` shares (proper field elements for the threshold-BLS scheme), and injects one share per node (e.g. as a Firebase-function secret). The full key briefly exists in the dealer — **testnet-only**, never mainnet. Outputs the **master public key** (client config) and optionally each node's **share-public-key** (for partial verification). See [07-roadmap](07-roadmap.md).

## 2. Encrypt (client, offline)

```
K          = random symmetric content key
inner      = AEAD(payload, K)                              // the content
K_wrapped  = ECIES(K, recipientPub)                        // recipient-gated
identity   = H("warden-cond-v1" ‖ jcs_rfc8785(condition))  // RFC 8785 canonicalization
outer      = IBE_encrypt(K_wrapped, identity, masterPub)   // (U,V,W); no network interaction
envelope   = { alg:"warden-v1", condition, outer, inner }  // see 04-envelope-format
```

## 3. Release (the on-chain-condition gate — the new part)

**Each node, on request for `identity` with condition `C`:**
1. Verify `H(C) == identity` (binding).
2. Verify `C.type/tier` is supported by this node.
3. **Evaluate `C` against finalized chain state** (read the named chain at ≥ `C.meta.finality` confirmations).
4. If `C` holds → return partial `sig_i = sk_i · H1(identity)` (publicly verifiable against the node's share-public-key).
   If not → `ConditionNotMet`.

**Client:** collect `t` valid partials → Lagrange-combine → `outerKey = msk · H1(identity)` → IBE-decrypt `outer` → `K_wrapped` → recipient ECIES-opens `K_wrapped` → `K` → AEAD-decrypt `inner` → plaintext.

**Idempotent + retryable.** Because the condition is monotonic and chain state is permanent, release can be requested **at any later time** and always succeeds once met. Nodes MAY cache released partials and serve them forever. → A temporarily-offline network causes **delay, not loss** (see [05-threat-model](05-threat-model.md)).

### Autonomous vs client-requested signing
Two designs (decide at PoC):
- **Client-requested (simplest):** nodes sign only when asked. Liveness depends partly on a requester existing. Good for testnet.
- **Autonomous (more drand-like):** nodes watch `MaktubCore` and proactively produce/store partials when `executed` flips, so a key is ready independent of any requester. Better liveness; more work. **Target for mainnet.**

## 4. Resharing (the permanence mechanism)

When operators join/leave (or for periodic share refresh), run **resharing**: old nodes deal their existing share as the *free coefficient* of a new polynomial; Lagrange interpolation gives new nodes shares of the **same master secret** → **the master public key is unchanged**, so all historical ciphertexts remain decryptable. New/old groups may be disjoint. `OldThreshold` MUST be specified (downgrade protection). This is what gives Warden **permanence with operator churn** — the property no vendor offered.

What resharing **can** change: membership, every share (rotate), the threshold. What it **cannot** change: the **master public key** (and thus the decryptability of past ciphertexts).

## 5. Finality policy (security-critical, no drand precedent)

A node must decide *when* an on-chain condition is "final enough" to release. A reorg that reverts `executed` **after** release is an **irreversible leak**. Policy:
- Read at the chain's **finalized/safe head**, or ≥ the chain's finality depth (Base: prefer L1-finalized state).
- The **minimum finality floor per chain is a federation-wide consensus parameter** — identical for every node, **not** configured locally per node. Per-node floors would make nodes disagree on whether a condition is final, stalling release (a node with a higher local floor blocks the threshold). `C.meta.finality` may override the floor **upward only**, never below it.
- Document the residual reorg risk for the chosen depth.

## 6. Verifiability

Every partial `sig_i` is a BLS signature verifiable against node `i`'s published share-public-key; the combined key verifies against the master public key. Clients reject invalid partials before combining (relay-trust-free, like drand-client). Misbehavior (a partial released for a condition that is *not* met) is **publicly provable** and can feed a slashing/eviction process in tokenized deployments (not required for the federation/public-good model).
