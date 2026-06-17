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

## JSON form (canonical)

```jsonc
{
  "alg": "warden-v1",
  "network": "<warden master-key id / network hash>",   // which Warden federation/master key
  "condition": {                                          // PUBLIC; hashed into the IBE identity
    "type": "contract",
    "chain": 8453,
    "address": "0x<MaktubCore>",
    "fn": "executed(uint256)",
    "args": ["<beatId>"],
    "test": { "cmp": "==", "value": true },
    "meta": { "finality": 32, "tier": 1 }
  },
  "outer": {                  // IBE(K_wrapped, H(condition)); K_wrapped = ECIES(K, recipientPub)
    "U": "<G1/G2 point>",     // commitment  r·G
    "V": "<bytes>",           // sigma  XOR  H2( e(P_pub, H1(identity))^r )
    "W": "<bytes>"            // K_wrapped XOR H4(sigma)   (Fujisaki–Okamoto, CCA)
  },
  "inner": {                  // AEAD(payload, K); recipient-only. Warden never touches this.
    "kwrap": "ecies-secp256k1",   // scheme used for K_wrapped (carried inside outer)
    "recipient": "<recipient pubkey / registry ref>",
    "ct": "<AEAD ciphertext of the payload under K>"
  }
}
```

`identity = H("warden-cond-v1" ‖ canonical_serialize(condition))` — recomputable by any node from the public `condition`, so the envelope need not store the identity.

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
