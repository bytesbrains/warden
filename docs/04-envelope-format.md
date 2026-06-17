# 04 — Envelope Format (`warden-v1`)

The envelope is the self-describing ciphertext a client produces and a recipient consumes. It carries the **public condition**, the **outer IBE wrapper** (condition-gated), and the **inner AEAD payload** (recipient-gated). It is versioned by `alg` so future schemes (incl. witness encryption) drop in with no consumer change.

## Construction (the double-wrap — read this first)

```
K          = random symmetric content key
inner.ct   = AEAD(payload, K)               // the content
K_wrapped  = ECIES(K, recipientPub)         // recipient-gated
outer      = IBE(K_wrapped, H(condition))   // condition-gated  → (U,V,W)
```

**Decrypt:** condition holds → combine ≥`t` partials → **IBE-open `outer`** → `K_wrapped` → **recipient ECIES-opens** `K_wrapped` → `K` → **AEAD-open `inner.ct`** → payload.

**Both gates are required.** The condition unlocks `outer` only to *reveal* `K_wrapped` (an ECIES ciphertext); only the recipient's private key turns `K_wrapped` into `K`. So even after the condition fires and the IBE key becomes public, **only the recipient can read content**, and Warden — which only ever causes `outer` to open — never sees `K` or the payload.

> **As implemented** (`warden/core/src/envelope.rs`): the BF-IBE block is a fixed 32 bytes, so `outer = IBE(K_wrapped, …)` is realized as a **hybrid** — `outer.ibe = IBE(obk, H(condition))` (gates a random 32-byte `obk`) and `outer.seal = AEAD_obk(K_wrapped)` (`obk` seals the ECIES-wrapped key). Logically identical to "IBE-gate `K_wrapped`". Hardening (per security review): both AEAD layers bind `domain ‖ network ‖ H(condition)` as **associated data** (tamper-evident + domain-separated), and the payload is **bucket-padded** to hide its length. AEAD = ChaCha20-Poly1305 (`nonce ‖ ct`); ECIES = secp256k1 with **HKDF info bound to `ephPub ‖ recipientPub`** (SEC1 shared-info). No recipient metadata is stored in the envelope. All domain tags / pad buckets are provisional — frozen with cross-language vectors before mainnet (#184).

## JSON form (canonical)

The wire form **as implemented** (`warden/core/src/envelope.rs`) — blobs are hex of the
compressed-canonical / `nonce ‖ ct` bytes:

```jsonc
{
  "alg": "warden-v1",
  "network": "<warden federation / master-key id>",       // bound into both AEAD layers
  "condition": {                                           // PUBLIC; hashed into the IBE identity
    "type": "contract",
    "chain": 84532,                                        // Base Sepolia (PoC)
    "address": "0x<MaktubCore>",
    "fn": "getHeartbeat(uint256)",                         // MaktubCore has no executed() getter…
    "args": ["<beatId>"],
    "word": 7,                                             // …executed is return field 7 (see 02)
    "test": { "cmp": "==", "value": true },
    "meta": { "finality": 32, "tier": 1 }
  },
  "outer": {                  // condition gate (hybrid IBE — see the "As implemented" note above)
    "ibe": "<hex>",           // IBE(obk, H(condition)) — the (U,V,W) ciphertext, canonical-serialized
    "seal": "<hex>"           // nonce ‖ AEAD_obk(K_wrapped), where K_wrapped = ECIES(K, recipientPub)
  },
  "inner": {                  // content layer; recipient-only. Warden never touches this.
    "ct": "<hex>"             // nonce ‖ AEAD_K(pad(payload))
  }
}
```

No recipient metadata is stored — the recipient is implicit in who can ECIES-open `K_wrapped`.
`identity = H("warden-cond-v1" ‖ jcs(condition))` — recomputable by any node from the public
`condition`, so the envelope need not store the identity. (`word` is omitted from the canonical
form when `0`, so single-value-getter conditions hash identically — see [02-condition-model](02-condition-model.md).)

## age-stanza form (drand/tlock-compatible, optional)

For interop with the tlock/age ecosystem, the outer layer MAY be expressed as an age recipient stanza (the DEK is timelocked to the condition):

```
-> warden <network-hash> <H(condition)>
<base64 outer IBE ciphertext of the file key>
--- <age MAC>
<age body: ChaCha20-Poly1305 over the payload>   // the "inner" content, recipient-bound
```

This mirrors tlock's `-> tlock {round} {chainHash}` stanza, swapping *round* for *H(condition)* and *chainHash* for the Warden *network hash*.

## Field notes

- **network** pins which Warden master key the `outer` layer is encrypted to (a federation may run multiple generations; resharing keeps a generation's master key stable — see [03-protocol](03-protocol.md) §4).
- **condition** is fully public and is the *only* thing nodes need to evaluate release; it must serialize deterministically.
- **outer** is the only thing the federation's release ever opens — IBE-opening it yields only `K_wrapped` (an ECIES ciphertext), never `K` or content.
- **inner.ct** is `AEAD(payload, K)`; `K` is recoverable only by ECIES-opening `K_wrapped` with the recipient's private key. Content confidentiality is independent of Warden entirely.

## Versioning

- `warden-v1` — Tier-1 conditions, BF-IBE over threshold BLS (BLS12-381); recipient-gating via an ECIES-wrapped content key (`K_wrapped`).
- Future `alg`s (e.g. `warden-we-v1` for a witness-encryption outer layer) replace **only** the `outer`/`condition` handling; `inner` and the integration are unchanged. Old envelopes remain decryptable forever.
