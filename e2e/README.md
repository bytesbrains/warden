# Veil end-to-end harness (Base Sepolia)

The capstone of Maktub #181 (WS-E): proves the whole Veil loop against **real on-chain
state** on Base Sepolia, tying together the dealer (WS-B), the node federation (WS-C), and
the client CLI (WS-D).

It asserts the three properties that *are* Veil:

1. **Undecryptable before** — a payload sealed to `getHeartbeat(beatId).executed == true` is
   unreadable while the Beat is active.
2. **Decryptable after** — once `executeHeartbeat(beatId)` flips `executed == true` and that
   state is **finalized**, ≥ `t` nodes release, the client combines, and the payload opens.
3. **Revocation** — a `deactivate`d Beat can never be executed, so it stays sealed forever.

`veil-e2e.mjs` runs in resumable phases (`setup` → wait ≥ interval → `trigger` → `revoke`, or
`all`), persisting a small state file so the expiry wait need not hold a process open.

## Prerequisites

- **A funded Base Sepolia key** (`PRIVATE_KEY`) — pays gas + the Beat creation fee, and is the
  `recipient`/`owner`. Must be a **registered, actively-staked executor** (`executeHeartbeat`
  is executor-only). Stake once via `scripts/test-heartbeat.js`; the harness only *checks* it.
- **A Base Sepolia RPC** (`BASE_SEPOLIA_RPC_URL`), ideally your own/authenticated.
- **A dealt federation + running nodes** — `cargo run -p warden-dealer -- --out fed -n 3 -t 2`,
  then `WARDEN_RPC_URL=<rpc> docker compose up --build` (see `../docker-compose.yml`).
- **The client binary** — `cargo build -p warden-cli` (default `WARDEN_BIN=warden/target/debug/warden`).
- **Node + ethers v6** — run from the repo root so `ethers` resolves from root `node_modules`.

## Run (from the repo root)

```bash
cargo run -p warden-dealer -- --out warden/fed -n 3 -t 2 --network warden-poc-local
WARDEN_RPC_URL=$BASE_SEPOLIA_RPC_URL docker compose -f warden/docker-compose.yml up --build -d
cargo build -p warden-cli

export BASE_SEPOLIA_RPC_URL=…    PRIVATE_KEY=…
node warden/e2e/veil-e2e.mjs setup     # creates a Beat, seals, asserts UNDECRYPTABLE
#   …wait ≥ INTERVAL (default 3600s — MIN_INTERVAL is 1h and immutable)…
node warden/e2e/veil-e2e.mjs trigger   # executeHeartbeat, asserts DECRYPTABLE once finalized
node warden/e2e/veil-e2e.mjs revoke    # deactivate a fresh Beat, asserts NEVER decryptable
# or: node warden/e2e/veil-e2e.mjs all  (long-running: sleeps through the interval)
```

Config via env (defaults target the 2026-06-16 Sepolia stack): `CORE`, `REGISTRY`, `REWARDS`,
`FEDERATION`, `NODES`, `WARDEN_BIN`, `STORE`, `INTERVAL`, `DECRYPT_TIMEOUT`.

## Finality policy & residual reorg risk (WS-E deliverable)

Nodes evaluate the condition at the **`finalized`** block tag (`WARDEN_FINALITY_TAG`, default
`finalized`; the federation-wide floor per [`../docs/03-protocol.md`](../docs/03-protocol.md) §5).
On Base (an OP-stack L2) the `finalized` tag only advances to state derived from
**L1-finalized** batches — so it lags `latest` by the L1 finality window (~2 epochs / ~13 min
on Ethereum, observed ~15–20 min on testnet). **This is why `trigger` polls for up to
`DECRYPT_TIMEOUT` (default 1500s) after `executeHeartbeat`** — release waits out finality.

Why this tag: a released decryption key is **irreversible** (once combined, it is out). Reading
at `finalized` means a reorg cannot revert `executed` *after* release — doing so would require
reorganising **already-finalized Ethereum L1 history**, which is outside the chain's security
model (a consensus safety fault, not a normal reorg).

Residual risk:
- **L1 finality failure** — only a catastrophic, slashing-level L1 safety fault could revert a
  finalized read. Accepted as out-of-model for the PoC.
- **Delay, not loss** — the finality lag only *delays* delivery after execution; the condition
  is monotonic and the client retries, so a key is never lost.
- **Misconfiguration** — `WARDEN_FINALITY_TAG=safe|latest` reintroduces reorg risk; the node
  warns loudly on startup and the floor is a federation consensus parameter, not a local knob.

## What's machine-verified vs operator-run

The crypto loop (seal → poll → verify → combine → open) is proven **offline** by
`warden/cli/tests/cli_flow.rs` against mock nodes. This harness adds the **live-chain** proof —
that `MaktubCore.executed`, read at finality, is what gates release — and inherently needs a
funded executor key and the ≥1h Beat expiry, so it is run by an operator, not in CI.
