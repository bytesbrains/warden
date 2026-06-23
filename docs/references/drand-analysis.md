# drand — Architecture, API & Cryptography Analysis (reference for Warden)

> **Fetched + analyzed 2026-06-17.** Targets **drand v2+**; v1→v2 deltas flagged inline. Every claim is URL-cited. Version-dependent or unconfirmed points (the G1 DST literal, the live League-of-Entropy threshold `t`, gossipsub-relay v2 status, exact tlock-js API drift) are flagged at their point of use. This document is the basis for Warden's reuse/replace decisions — see [`../01-architecture.md`](../01-architecture.md) and [`../03-protocol.md`](../03-protocol.md).

This tells us exactly what to **reuse** from drand and what to **replace** to build a threshold-IBE network that releases a decryption key when an on-chain condition (e.g. `MaktubCore.executed(beatId) == true` on Base) holds, instead of when a time/round is reached. §§1–7 are sourced reference; §§8–9 are the load-bearing analysis; the final summary is the decision list.

---

## 1. System architecture

**What drand is.** A distributed randomness beacon **daemon** in Go. Linked servers collectively produce publicly-verifiable, unbiased, unpredictable random values at fixed intervals using bilinear pairings + threshold cryptography ([github.com/drand/drand](https://github.com/drand/drand); [drand.love/about](https://www.drand.love/about/)). The daemon is wrapped in screen/tmux/systemd ([operator CLI](https://docs.drand.love/operator/drand-cli/)).

**Node lifecycle.** (1) **Setup** — each node generates a long-term keypair; public keys collected into a **group file**; (2) **DKG** — nodes run distributed key generation collectively; (3) **Randomness generation** — the beacon loop starts automatically when the DKG finishes ([README](https://github.com/drand/drand/blob/master/README.md); [operator CLI](https://docs.drand.love/operator/drand-cli/)). State lives under `$HOME/.drand/`. Each node runs a **beacon process keyed by Beacon ID**, so one daemon can run multiple beacon chains ([specification](https://docs.drand.love/docs/specification/)).

**Three network surfaces** ([operator CLI](https://docs.drand.love/operator/drand-cli/)): **Control plane** (local management; control port default **8888**, localhost only, must not be exposed); **Private plane** (inter-node gRPC via `--private-listen`); **Public plane** (optional public consumption via `--public-listen`, deprecated in favor of a standalone **relay**).

**Networking.** Node↔node is **gRPC** over private-listen (protobuf, routed by Beacon ID) — **not** libp2p for the core DKG/beacon path ([specification](https://docs.drand.love/docs/specification/)). Node→public is via **relays**: libp2p **PubSub gossipsub**, HTTP, historically S3 ([gossipsub](https://docs.drand.love/developer/gossipsub/)). League of Entropy runs three first-tier gossip relays — `api.drand.sh`, `api2`, `api3`. *v2:* client/relay tooling split into `github.com/drand/go-clients`.

**Beacon chain & chain hash.** Each beacon = **round number, previous signature, final signature**; chained mode builds on the previous ([specification](https://docs.drand.love/docs/specification/)). `round = floor((now − genesis)/period) + 1`. A chain's identity = `Info` {public key, period, genesis time, genesis seed, beacon ID}, identified by a **SHA-256 chain hash**, stable across node churn (quicknet hash `52db9ba70e0c…84e971`). Public network run by the **League of Entropy**.

## 2. Cryptography

**Scheme/curve.** **Threshold BLS on BLS12-381** ([cryptography](https://docs.drand.love/docs/cryptography/)). ≥`t` partial signatures reconstruct the collective signature; threshold > 50% of nodes. BLS is **deterministic** → unbiasable, fork-resistant.

**Group placement (scheme-dependent):** historical `pedersen-bls-*`: sigs on **G2**, pubkeys on **G1**. Modern **quicknet** `bls-unchained-g1-rfc9380`: the swap — **sigs on G1 (48 B), collective pubkey on G2 (96 B)** — halves beacon size + lowers on-chain verify cost ([quicknet-is-live](https://docs.drand.love/blog/2023/10/16/quicknet-is-live/)). *v1→v2:* fastnet sunset → quicknet (full RFC-9380 hash-to-curve).

**Chained vs unchained.** Chained signs `m = H(round ‖ prev_sig)`; **unchained** signs `m = H(round)` only. **Unchained is required for tlock** — you can predict the message a future round will sign (but nothing else). This predictability-of-identity is the property Warden must reproduce. Hash-to-curve = **RFC 9380**. *Caveat:* the spec exposes the G2 ciphersuite DST but not the G1 (quicknet) one — confirm in `kyber` source if load-bearing.

**`github.com/drand/kyber`** provides modular sub-packages: `sign/bls`, **`sign/tbls`** ((t,n)-threshold BLS, what the beacon uses), `sign/bdn`, `share` + **`share/dkg`**, `share/vss/pedersen`, `pairing/...` (BN254 + BLS12-381), **`encrypt/ibe`** (tlock). Concrete BLS12-381 from `kyber-bls12381` (wraps `kilic/bls12381`). kyber's README warns it needs independent review before security-critical use.

## 3. DKG protocol

**Purpose.** Create one distributed BLS private key across `n` nodes so any `t` can sign but no smaller coalition can — **no single party knows the full secret** ([DKG blog](https://docs.drand.love/blog/2023/09/08/distributed-key-generation/)). drand uses **Pedersen's DKG** from parallel **Feldman VSS**: every node deals shares + broadcasts commitments; each node sums received shares into its final share.

**Phases** (kyber `share/dkg`): **Deal** (ECIES-encrypt shares per recipient + commitments, broadcast signed `DealBundle`) → **Response** (validate; success/complaint) → **Justification** (only on complaints; QUAL set) → **Finish** (compute final share + distributed public key). Driven by timeouts or **FastSync**. Packets BLS-authenticated; shares ECIES-encrypted; 32-byte nonce prevents cross-run replay.

**v2 DKG redesign** = the **orchestration layer** (crypto phases largely unchanged): commands-vs-packets split, a **durable boltDB state machine** (survives restarts), explicit **joiners/remainers/leavers** ([PR #1081](https://github.com/drand/drand/pull/1081); [v2 post-mortem](https://docs.drand.love/blog/2025/03/21/drand-v2-0-postmortem/)). *Coordination nuance:* the share ceremony is **coordinator-based, not gossip** ([specification](https://docs.drand.love/docs/specification/)); only the v2 proposal/acceptance layer gossips state.

**v2.0 security flaw (heed this).** v2.0.0 merged operator `Command` RPC with node-to-node RPC on one service; exposing it let a malicious operator **chain reshares to iteratively reduce the threshold to their own nodes** — full master-key recovery. Fixed in v2.1.0 by splitting **`DKGControl`** (local) from **`DKGPublic`** ([post-mortem](https://docs.drand.love/blog/2025/03/21/drand-v2-0-postmortem/)). **Warden must keep operator-command surfaces strictly local.**

**CLI:** `drand dkg generate-proposal|init|join|accept|reject|execute|abort|status|reshare`. **Group file** (`group.toml`): node list, threshold, period, beacon ID, scheme ID, genesis time/seed, the **distributed public key**, TransitionTime. **Threshold** defaults to `⌊n/2⌋+1`. **No production trusted-dealer mode** — kyber VSS can be single-dealer-driven for *testing* only.

## 4. Resharing

**Purpose (critical for Warden permanence).** Add/remove nodes and change threshold **while keeping the same public-facing distributed key** → chain identity continues uninterrupted. After it finishes, new-group nodes hold **new shares for the same group public key**.

**Mechanism.** kyber's `dkg` runs either a fresh DKG or a **reshare** (fills `OldNodes`; existing nodes provide `Share`, `PublicCoeffs`, `OldThreshold`). **`OldThreshold` MUST be specified to avoid a downgrade attack** (the v2.0 bug). Each old node builds its private polynomial so the **free coefficient is its existing share**; Lagrange-interpolating valid sub-shares reconstructs new shares of the *same* secret without anyone reconstructing it → "the distributed public polynomial changes but not the free coefficient which is the public key." Old/new groups may be **disjoint**.

**Can change:** membership, all shares (rotate), threshold (with downgrade protection). **Cannot change:** the **distributed public key**, chain identity, genesis, period. *v1→v2:* `drand share` → the `drand dkg reshare`/`join`/`accept`/`execute` state machine; core reshare math unchanged.

## 5. tlock — the closest existing thing to Warden

**tlock** encrypts "to a future round R"; decryptable by anyone, offline, no trusted party at decrypt time, once drand publishes round R's BLS signature ([eprint 2023/189](https://eprint.iacr.org/2023/189); [timelock docs](https://docs.drand.love/docs/timelock-encryption/)).

**Construction — Boneh–Franklin IBE over threshold BLS.** The beacon is *already* an IBE Private-Key-Generator it doesn't know it's running: **IBE identity = round number; BLS signature over that round = the IBE private key** for that identity.

```
U = r·G                                   // commitment
V = sigma XOR H2( e(P_pub, Q_id)^r )      // Q_id = H1(identity); P_pub = group public key
W = M     XOR H4( sigma )                 // r = H3(sigma||M)  => Fujisaki–Okamoto CCA
Decrypt with round sig (= d_id = msk·Q_id):
  gt = e(U, sig) = e(P_pub, Q_id)^r ;  sigma = V XOR H2(gt) ;  M = W XOR H4(sigma) ; verify U==H3(sigma||M)·G
```
A BLS sig on `m` is `msk·H(m)`; the BF-IBE key for `id` is `msk·H1(id)`. If `id = round` is hashed the same way drand signs, **the round signature literally is the IBE private key** — no extra network work.

**Identity model:** identity = the **round number** (`uint64`); master public key = the **drand group public key** (unchanged across resharing). **The single most important fact for Warden: the identity is whatever string the network signs. Nothing in the IBE math requires it to be a round number.**

**Unchained mandatory** (chained's future message is unpredictable). **Threshold:** each node emits partial `sig_i = sk_i·H(R)`; `t` combine by Lagrange-in-the-exponent → `msk·H(R)`. Backed by `kyber/sign/tbls`. **Curve (quicknet):** IBE private key (=sig) in **G1**, master public key in **G2**, identity hashes into **G1**.

**Envelope/APIs:** hybrid — message age-encrypted (ChaCha20-Poly1305); tlock timelocks the file key as an **age stanza** `-> tlock {round} {chainHash}`. Go `github.com/drand/tlock` (`Encrypt(dst,src,round)` / `Decrypt`, `ErrTooEarly`); TS `tlock-js` (`timelockEncrypt/Decrypt`). Rust: `thibmeu/tlock-rs`, `ideal-lab5/timelock`. *Stated horizon ≥5 yrs; not quantum-resistant.*

**Transforming tlock TIME→EVENT (the core analysis).** *Stays identical:* the entire IBE/pairing/threshold/age layer — condition-agnostic, **reuse verbatim**. *Changes:* (1) **identity round → `H(condition)`** (domain-separated `H(chainId ‖ contract ‖ beatId ‖ "executed")`; mathematically free, preserves predictability); (2) **release: time-elapse → on-chain boolean** — each node signs `H1(condition)` **iff** it verifies the condition on Base (e.g. `executed(beatId)==true` for a Maktub Beat — the only substantive new component, a per-signer **condition oracle**); (3) **domain-separation against replay/grinding** (identities now attacker-influenceable); (4) **nothing on-chain changes** — the consumer's contract (e.g. `MaktubCore`) stays immutable, merely exposing the gated state.

## 6. APIs

**Transports:** HTTP (dominant), libp2p PubSub, gRPC. v2 HTTP has **v1** (chain-hash-prefixed) and **`/v2`** (`/v2/beacons/{beaconID}/rounds/{round|latest}`). v1 endpoints: `GET /chains`, `/{hash}/info`, `/{hash}/public/latest`, `/{hash}/public/{round}`, `/health`. Beacon JSON: `{round, randomness, signature, previous_signature}` (unchained omits `previous_signature`). Public relays: `api.drand.sh` (+`/v2`), `api2/3`, `drand.cloudflare.com`. **Clients** (Go `drand/client`, JS `drand-client`) verify each beacon against the chain hash + public key — **relay-trust-free**. Third-party Rust client `drand_core`. **Operator/control API:** gRPC on **8888**, localhost-only.

## 7. Operator / deployment reality

**Requirements:** ~micro compute, **8 GB RAM**, ≥**32 GB** storage, ≥**1 Mbps** dedicated bandwidth. **Run/join:** `generate-keypair` → `start --private-listen … --control …` → `dkg generate-proposal/init/join/execute`; beacon starts when DKG finishes. **Config under `$HOME/.drand/`.** **TLS (v2):** drand does **not** terminate TLS — run behind a reverse proxy (nginx `grpc_pass`) — "the only method of deploying drand." **Ports:** control 8888 (localhost), private/gRPC `--private-listen` (TLS-fronted), public HTTP, metrics. **League of Entropy:** **15 independent organizations**; founded 2019 (Protocol Labs, Cloudflare, EPFL, Kudelski, Univ. of Chile). *Exact LoE threshold `t` not published — read live `group.toml`/`/info`.*

## 8. Reuse vs replace for Warden

**KEEP (reuse ~verbatim):** threshold BLS on BLS12-381; BF-IBE/tlock crypto (`U,V,W`, FO, pairing decrypt); DKG (Pedersen/Feldman, kyber `share/dkg`); **resharing** (old-share-as-free-coefficient ⇒ same master public key — the permanence mechanism); unchained/predictable-identity semantics; age hybrid envelope (new `warden` stanza); control/private/public plane separation + local-only operator commands; client-side verification against the master public key.

**REPLACE / ADD:** round scheduler → **Base condition-watcher** (event-driven, per-signer); identity round → **domain-separated `H(condition)`**; periodic proactive signing → **on-demand / autonomous condition-keyed release**; (new) **finality handling** for the chain read; randomness firehose → **no firehose** (pull-by-condition); beacon DB → **idempotent per-condition released-key store**.

**Go vs Rust.** Rust primitives exist: BLS12-381 + RFC-9380 (arkworks / `blst` / `blstrs`); threshold BLS (`blsful`, Kudelski-audited); Shamir/VSS (`vsss-rs`); DKG (`gennaro-dkg`, Kudelski-audited); BF-IBE timelock (`thibmeu/tlock-rs`; **`ideal-lab5/timelock`** — beacon-agnostic, the closest fit). Re-build: the daemon, networking, the v2-style DKG/reshare orchestration, and the **Base condition-watcher + finality** + on-demand request protocol. Budget a security audit — Rust threshold/DKG/IBE crates are audited-or-auditing at best.

## 9. Gotchas / limits

1. **drand is inherently periodic/time-driven** — Warden has no clock and no firehose; signing is sparse, conditional, event-triggered. You're running an **on-demand threshold-IBE key-release service**, not a beacon.
2. **Proactive vs on-demand** — a Warden key exists only after the condition is met *and* `t` partials are gathered; needs a request/response (or autonomous-watch) protocol drand lacks.
3. **The condition oracle becomes part of the trust model** — threshold now also covers **correct on-chain predicate evaluation**; a lying/eclipsed RPC could release early; each signer needs its own trustworthy Base view + a **finality policy** (reorg-after-release = irreversible leak). New, security-critical, no drand analog.
4. **Identity is attacker-influenceable** — domain-separate `H1` and bind chain/contract/beatId, or a partial for one item could open another / be ground.
5. **Resharing preserves the master key — but a leaked share is permanent** — any eventual `msk` reconstruction breaks **all** past+future ciphertexts (heavier for Warden: ciphertexts may sit dormant for years).
6. **No quantum resistance** (~5-yr horizon) — disclose for decades-long payloads; track PQ migration.
7. **Rust threshold/DKG/IBE crates mostly unaudited/mid-audit** — own security review required before mainnet.
8. **gossipsub-relay v2 status under-documented; exact LoE threshold unpublished** — read live config, don't hardcode.

## What this means for Warden (decision list)

- **Reuse the whole IBE/threshold-BLS/DKG/reshare stack** — math is condition-agnostic; only the *identity string* and the *release gate* change.
- **Swap the gate, not the cryptography:** identity `H(round)` → `H(chainId ‖ contract ‖ condition)` (e.g. `H(chainId ‖ MaktubCore ‖ beatId ‖ "executed")` for a Maktub Beat); trigger "time ≥ R" → "the condition holds, per-signer verified" (e.g. `executed(beatId)==true`). The only fundamentally new component.
- **Keep resharing** — it gives permanence-with-churn (master public key survives turnover). Core, not optional.
- **Replace the round scheduler with a Base condition-watcher + finality policy**; reorg-after-release is an irreversible leak — choose confirmations conservatively.
- **Move to on-demand / autonomous condition-keyed release**; decide autonomous-watch (better liveness) vs client-requested.
- **Rust is viable** (arkworks-native or blst-native stacks; `ideal-lab5/timelock` closest); budget an audit.
- **Inherit drand's v2.0 lesson:** strict control/node plane separation + `OldThreshold` downgrade protection on every reshare.

### Riskiest unknowns
1. **Finality/reorg policy for the Base read** — no drand precedent; highest-stakes new decision.
2. **On-demand orchestration & liveness** — who triggers signing; guaranteeing a met condition's key gets released and stays released forever.
3. **Long-horizon key security** — non-PQ + immutable master key ⇒ one eventual `msk` break exposes decades of dormant ciphertexts; consider a new-generation migration story (opt-in re-encryption, e.g. an immutable-V2-style pattern).
4. **Identity domain-separation** — wrong `H1` binding enables cross-item key reuse / grinding.
