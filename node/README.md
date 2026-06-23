# warden-node (`wardend`)

A **Warden node** (Phase 0 PoC). Holds **one** share of the
federation master key and releases its threshold-BLS **partial** for a condition's identity
**iff** the condition holds against **finalized** Base Sepolia state. It never sees plaintext
and never holds the master key — it only ever releases *timing*.

> ⚠️ **Not audited. Not for production.** All-ours testnet = zero security by design
> (`../docs/05-threat-model.md`).

## The security-critical part

The novel piece is the **condition-watcher + finality policy** (`src/eval.rs`):

- **Identity is derived, never supplied.** The request carries the full `condition`; the node
  computes `identity = H(condition)` itself and signs *that*. It cannot be tricked into
  signing an attacker-chosen identity.
- **Finalized reads only.** Conditions are evaluated at the `finalized` block tag (Base:
  L1-finalized) — the conservative, reorg-safe choice and the intended federation-wide floor.
  A reorg that reverts a condition *after* release would be an irreversible leak, so anything
  but `finalized` is refused outside local testing.
- **Fail-closed.** Any RPC failure is *transient* (the client retries); the node never
  releases on an unreadable or ambiguous chain.
- **Chain + tier gates.** Refuses conditions for a different chain, or tier-2 (external-data)
  conditions. Phase 0 evaluates tier-1 `contract` + `block` only.

## HTTP API

| Method | Path | Body | Response |
|---|---|---|---|
| `POST` | `/partial` | `{ "condition": {…} }` | `200 {released:true, index, identity, partial}` · `200 {released:false, reason:"condition_not_met"}` · `400` malformed · `422` unsupported/invalid (no retry) · `503` chain unavailable (retry) |
| `GET` | `/health` | — | `200 {"status":"ok"}` |
| `GET` | `/info` | — | `200 {index, network, chain_id, finality, mpk}` (no secrets) |

## Config (environment)

| Var | Required | Default | Meaning |
|---|---|---|---|
| `WARDEN_SHARE_FILE` | yes | — | Path to this node's `node-<i>.json`. |
| `WARDEN_RPC_URL` | yes | — | Base Sepolia JSON-RPC endpoint (read-only). |
| `WARDEN_LISTEN` | no | `0.0.0.0:8080` | HTTP bind address. |
| `WARDEN_CHAIN_ID` | no | `84532` | Chain this node watches. |
| `WARDEN_FINALITY_TAG` | no | `finalized` | `finalized` \| `safe` \| `latest` (only `finalized` is reorg-safe). |

## Example consumer condition: Maktub Veil (`executed`)

A worked example of a real consumer condition. `MaktubCore` exposes no `executed(uint256)`
getter; execution status is the 8th field of `getHeartbeat(uint256)`'s return tuple. So this
release condition reads return **word 7**:

```json
{
  "type": "contract", "chain": 84532,
  "address": "0xb603C96D089F64Ac487EE0bdaE97D49848F86133",
  "fn": "getHeartbeat(uint256)", "args": ["<beatId>"], "word": 7,
  "test": { "cmp": "==", "value": true },
  "meta": { "finality": 32, "tier": 1 }
}
```

Revocation is automatic: a `deactivate`d beat can never be `execute`d, so `executed` stays
`false` forever → the node never releases → the payload stays gibberish.

## Run

```bash
# 1. Deal a federation (see warden-dealer).
cargo run -p warden-dealer -- --out ../fed -n 3 -t 2 --network warden-poc-local
# 2. Run one node directly:
WARDEN_SHARE_FILE=../fed/shares/node-1.json WARDEN_RPC_URL=https://sepolia.base.org \
  cargo run -p warden-node
# 3. Or the whole 3-node federation in containers (see ../docker-compose.yml):
WARDEN_RPC_URL=https://sepolia.base.org docker compose -f ../docker-compose.yml up --build
```

```bash
curl -s localhost:8080/info
curl -s -XPOST localhost:8080/partial -d '{"condition":{ … }}'
```
