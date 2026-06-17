# 06 — Operator Manual (and partner one-pager)

For organizations (universities, companies) considering running a **Warden node**. The hardware ask is small; the real commitment is **reliability, key custody, and longevity**.

## What you're running

A `wardend` node holds **one share** of the federation's master key (you never hold the whole key) and answers: *"give me your partial decryption for condition C"* — returning it **only if** C holds on-chain (e.g. a Maktub Beat has gone silent). You can never read anyone's content (it stays encrypted to the recipient); you only ever release **timing**.

## Resource requirements

| Resource | Requirement | Notes |
|---|---|---|
| CPU | 1–2 vCPU | One threshold-BLS partial per request — microseconds of compute |
| RAM | 2–8 GB | (drand's documented footprint range) |
| Disk | 20–50 GB | Key share, config, released-partial cache, logs |
| Bandwidth | ≥1 Mbps dedicated | Low traffic; well-connected infra preferred |
| OS / hardware | Any always-on Linux box / VM / container | No GPU, no special hardware; shares existing infra fine |
| **Chain access** | testnet: a **Base RPC** (provider). **Mainnet: own node or independent, authenticated, diverse RPCs (mandatory)** | A shared/compromised public RPC could spoof `executed==true` and trick a threshold into early release (eclipse attack) — see note below. Own node ≈ 1–2 TB SSD. |

**Cost:** minimum viable ≈ a cheap always-on VM + an RPC key (~$10–50/mo). Fully self-sovereign (own Base node) ≈ $150–350/mo. **No token, no GPU, no special hardware.**

## Network planes & ports (inherit drand discipline)

- **Control plane** — local-only management (`wardenctl` → daemon on a localhost port). **Never expose to the internet.** (drand's v2.0 breach came from violating this.)
- **Private plane** — node↔node for DKG/resharing; behind a TLS reverse proxy (Warden does not terminate TLS itself — run nginx in front).
- **Public plane** — clients fetch partials (HTTP/relay); read-only, partials are publicly verifiable.

## Setup (sketch — finalized at PoC)

1. `wardenctl keygen` → long-term node keypair.
2. Provide your **Base RPC endpoint** + finality preference.
3. Join the **DKG ceremony** (mainnet) or receive your dealer-issued share (testnet).
4. `wardend start` behind your TLS proxy; expose only the public partial-serving endpoint.
5. Configure monitoring/alerting on liveness + Base-RPC health.

## Ongoing responsibilities (the real ask)

- **~24/7 uptime + monitoring** so `t` nodes are reachable when conditions fire. (A brief outage only *delays* delivery — never loses it — but availability still matters, especially for urgent use cases.)
- **Key-share custody** — protect your share (HSM recommended; backups + succession plan). Losing it triggers a reshare; many simultaneous losses threaten the master key.
- **Participate in resharing** when the operator set changes — occasional coordinated ceremonies (keeps the master key stable while membership churns).
- **Longevity** — commit to running for years. This is why **institutions that persist** (universities) are ideal partners.

Effort once set up: **a few hours/month** of part-time-sysadmin attention. No dedicated team.

## ⚠️ Condition-source integrity (mainnet)

The federation's honest-majority guarantee assumes each node sees the **true** chain state. If many operators rely on the **same** public RPC provider, a compromise or **eclipse** of that provider can spoof `executed(beatId)==true` and trick a *threshold* of nodes into releasing partials early — breaking confidentiality without breaking any crypto. Therefore on **mainnet**:
- Run your **own Base node**, or use **independent, authenticated, jurisdictionally-diverse** RPC endpoints — operators **must not** converge on a single provider.
- Read at the **federation-wide finality floor** (a consensus parameter, not a local setting — see [03-protocol](03-protocol.md) §5), so a reorg can't be used to induce an early-then-reverted release.

## Independence is the security (not node count)

Security comes from operators being **genuinely independent** — different organizations, jurisdictions, and infrastructure — so no single subpoena, coercion, or failure reaches a threshold. Even ~3–5 independent operators give a real `t`-of-`n`. The goal is **diversity and longevity**, not a data center.
