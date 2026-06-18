# Warden federation on fly.io (testnet)

Deploys the Warden federation as **N separate fly.io apps** — one `wardend` + one secret key
share each, one public `*.fly.dev/partial` endpoint each. This is the faithful production
topology (distinct operators/endpoints, isolated shares) and a clean hand-off: a real operator
later runs the **same image** on their own fly account or VM with their share.

> ⚠️ **Testnet only — zero timing-security.** An all-ours federation cannot provide the timing
> guarantee (D-036): whoever runs ≥ `t` nodes can release early. Real security requires the N
> nodes to be **independent institutions**. This setup is about ops faithfulness + hand-off, not
> trust. Content confidentiality (the recipient's key) is unaffected and remains real.

## Why separate apps (not sub-paths on one box)
Each node holds **one** share; co-locating all shares on one machine makes a single compromise a
total key compromise and defeats the threshold even for testing. Separate apps isolate the
shares, exercise the real cross-node topology, and let you drop in a real operator per node
without re-architecting. (See the deploy discussion; D-036.)

## One-time per release

```bash
# 1. Deal the federation (operator, local — produces SECRET shares + the public federation.json).
#    /fed is git-ignored (warden/.gitignore); never commit shares.
cargo run -p warden-dealer -- --out warden/fed -n 5 -t 3 --network base-sepolia

# 2. Deploy 5 apps, one share each. Sets the share + RPC as fly secrets (encrypted at rest).
fly auth login
FED_DIR=warden/fed BASE_SEPOLIA_RPC_URL='https://base-sepolia.<provider>/<key>' \
  warden/deploy/fly/deploy.sh 5 maktub-warden-node

# 3. The script prints the NODES endpoint list + points at federation.json. Give clients both.
curl https://maktub-warden-node-1.fly.dev/health    # → {"status":"ok"}
```

## What goes where
| | Value | How |
|---|---|---|
| Secret share (per node) | `shares/node-<i>.json` → base64 | `fly secrets set WARDEN_SHARE_B64=…` (deploy.sh) |
| Base RPC (read-only) | `WARDEN_RPC_URL` | `fly secrets set` (deploy.sh) — secret (may carry an API key) |
| Chain / finality / listen | `84532` / `finalized` / `0.0.0.0:8080` | `[env]` in `fly.node.toml` (non-secret) |
| Public mpk + share pubkeys | `federation.json` | dealer output; hand to clients (not on fly) |

`entrypoint.sh` decodes `WARDEN_SHARE_B64` to a `0600` `/tmp/share.json` and execs `wardend`.
The node reads chain state at the **finalized** tag (reorg-safe) and answers `POST /partial` only
when the condition holds.

## Files
- `Dockerfile` — builds `wardend` (rust:1.83, matches the pin); slim runtime.
- `entrypoint.sh` — share-secret → file, then `exec wardend`.
- `fly.node.toml` — per-node app config (always-on, `/health` check, `internal_port 8080`).
- `deploy.sh` — deploys N apps + stages per-node secrets; prints the NODES list.

## Hand-off to a real operator (mainnet path)
Give the institution: the image (this `Dockerfile`) + their share + the RPC requirement. They run
`wardend` on their own infra (fly app, VM, or container platform), publish their `/partial` URL,
and it joins the federation. Nothing here is fly-specific beyond convenience.
