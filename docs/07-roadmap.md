# 07 — Roadmap

Build private through testnet, **open-source before partners/mainnet** (openness is part of the trust model and the adoption story — no serious operator runs a black box), audit before real secrets.

## Phase 0.5 — Architecture spike — ✅ DONE → [08-architecture-decision](08-architecture-decision.md)
**Outcome:** **Gen-1 = DKG threshold-IBE (tlock-derived)** — the only audited, production-mature option; the challengers (silent-setup, SWE) are GGM/iO/trusted-CRS + unaudited PoC, unsafe for seed-phrase secrets today. **Condition-gating is app-layer in all three**, so the choice was made on maturity + longevity. Roadmap: SWE = gen-2 (longevity-optimal), traitor-tracing = a layer to close W1 when production-ready, witness-encryption = gen-3 — all via the `alg` envelope. **Re-run this spike semi-annually.**

## Phase 0 — PoC — ✅ CODE-COMPLETE (#181)
**Goal:** prove the whole loop end-to-end, locally, on the gen-1 (DKG-IBE) architecture with trusted-dealer shares. **All workstreams merged** (`warden/core`, `dealer`, `node`, `cli`, `e2e/`).
- Rust core: BF-IBE encrypt/decrypt + threshold-BLS partial/combine + the `warden-v1` double-wrap (on `arkworks` BLS12-381). [#182, #186]
- **Trusted-dealer** key setup (skip DKG) + federation file format; 3 nodes via `docker-compose`. [#200]
- Condition evaluator (`wardend`): reads the Beat's `executed` flag on **Base Sepolia** at the `finalized` tag. Note: `MaktubCore` exposes no `executed(uint256)` getter — status is field 7 of `getHeartbeat(uint256)`, so the condition uses `word: 7` (see [02-condition-model](02-condition-model.md)). [#201]
- CLI client (`warden`): double-wrap encrypt → publish CID → fetch partials → combine → decrypt, retry-until-released. [#202]
- E2E harness (`warden/e2e/`): create+execute a Beat → assert sealed-then-readable → deactivate → assert never. [#203]
- **Exit:** a sealed payload is unreadable before `executed`, readable after, on real Sepolia state. *Crypto loop proven offline (`cli/tests/cli_flow.rs`); the live Sepolia run is operator-driven (funded staked-executor key + ≥1h Beat expiry).*

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
- ~~Fork drand (Go) vs fresh Rust core~~ — **resolved (Phase 0): fresh Rust core** (`warden/core`, `arkworks` BLS12-381). One core → node + future WASM/Dart-FFI.
- Autonomous (watch-and-sign) vs client-requested partial release — **Phase 0 ships client-requested** (`wardend` signs on `POST /partial`); autonomous is the mainnet target (revisit Phase 1, per [03-protocol](03-protocol.md) §3).
- Finality depth floor for Base — **Phase 0 reads at the `finalized` tag** (L1-finalized; federation-wide floor). Confirm/parameterize before mainnet ([03-protocol](03-protocol.md) §5).
- Federation incentive model (public-good/no-token vs. later staking) — current lean: **no token**.
