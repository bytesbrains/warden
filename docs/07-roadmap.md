# 07 — Roadmap

Build private through testnet, **open-source before partners/mainnet** (openness is part of the trust model and the adoption story — no serious operator runs a black box), audit before real secrets.

## Phase 0.5 — Architecture spike — ✅ DONE → [08-architecture-decision](08-architecture-decision.md)
**Outcome:** **Gen-1 = DKG threshold-IBE (tlock-derived)** — the only audited, production-mature option; the challengers (silent-setup, SWE) are GGM/iO/trusted-CRS + unaudited PoC, unsafe for seed-phrase secrets today. **Condition-gating is app-layer in all three**, so the choice was made on maturity + longevity. Roadmap: SWE = gen-2 (longevity-optimal), traitor-tracing = a layer to close W1 when production-ready, witness-encryption = gen-3 — all via the `alg` envelope. **Re-run this spike semi-annually.**

## Phase 0 — PoC (weeks)
**Goal:** prove the whole loop end-to-end, locally. (Uses the architecture chosen in 0.5 — if silent setup wins, there is **no DKG** to build; if DKG-federation is retained, use trusted-dealer shares for the PoC.)
- Rust core: BF-IBE encrypt/decrypt + threshold-BLS partial/combine (reuse `tlock-rs` / `ideal-lab5/timelock` / `blsful`).
- **Trusted-dealer** key setup (skip DKG); 3–5 nodes via `docker-compose`.
- Condition evaluator: read `MaktubCore.executed(beatId)` on **Base Sepolia** (already deployed).
- CLI client: double-wrap encrypt → create+execute a Beat → fetch partials → combine → decrypt.
- **Exit:** a sealed payload is unreadable before `executed`, readable after, on real Sepolia state.

## Phase 1 — Testnet federation (a few weeks)
**Goal:** a faithful distributed testnet.
- 5 nodes as real separate instances — **5 subdomains/VMs** or **5 Firebase Functions** (dealer-injected shares; see [03-protocol](03-protocol.md)).
- TLS termination via reverse proxy; control plane local-only.
- TS SDK (WASM) client; begin Dart-FFI client.
- `re-wrap-on-check-in` flow; autonomous-vs-requested signing decided here.
- **Honest disclosure:** all-ours = **zero security**; testnet only, no real secrets.
- **Exit:** Maktub mobile/web can create and open a Veil beat on testnet.

## Phase 2 — Open-source + audit (gate before mainnet)
- **Publish the source (MIT).** (The *spec* — this `docs/` — is public from day one; the implementation opens here.)
- External security audit: crypto core + DKG/resharing + condition/finality logic + node.
- Finalize **finality/reorg policy**, identity domain-separation, control-plane hardening (drand v2.0 lessons).
- **Exit:** audited, open, documented.

## Phase 3 — Mainnet federation
- Recruit **independent** operators (universities + partner companies; diversity > count).
- Real **DKG** ceremony (no dealer); resharing tested.
- Dart-FFI mobile client production-ready.
- Multi-network redundancy plan (k-of-N across independent Warden generations/networks) + permanence governance.
- **Exit:** Veil live on Maktub mainnet, honestly described.

## Phase 4 — Public good / adoption
- Generalized condition tiers (cross-chain, events, eventually Tier-2 oracle).
- Onboard third-party apps (the "others adopt it" thesis).
- Track **witness encryption** semi-annually as the trustless successor (swap via `alg`, no consumer change).

## Open decisions to resolve along the way
- Fork drand (Go) for the node vs. fresh Rust core. **Current lean: Rust core** (one core → node + web + mobile). Revisit if a Rust DKG proves too risky to audit (fallback: drand-Go DKG).
- Autonomous (watch-and-sign) vs client-requested partial release.
- Finality depth floor for Base.
- Federation incentive model (public-good/no-token vs. later staking) — current lean: **no token**.
