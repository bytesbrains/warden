#!/usr/bin/env bash
# Deploy the Warden federation as N separate fly.io apps — one wardend + one secret share each.
#
# ⚠️ TESTNET ONLY. An all-ours federation has ZERO timing-security (D-036): real security needs
#    the N nodes run by INDEPENDENT institutions on their own infra. This is the faithful
#    topology + a turnkey hand-off (a real operator runs the same image on their fly/VM).
#
# Prereqs (operator, run locally — handles secret share material):
#   1. flyctl authed:  fly auth login
#   2. A dealt federation (the dealer writes SECRET shares + the public federation.json):
#        cargo run -p warden-dealer -- --out <FED_DIR> -n 5 -t 3 --network base-sepolia
#      → <FED_DIR>/shares/node-1.json … node-5.json   (SECRET — never commit; warden/.gitignore covers /fed)
#      → <FED_DIR>/federation.json                     (public: mpk + share pubkeys + n + t)
#   3. A read-only Base Sepolia RPC URL (treated as a secret — may carry an API key).
#
# Usage (from anywhere):
#   FED_DIR=warden/fed BASE_SEPOLIA_RPC_URL='https://…' \
#     warden/deploy/fly/deploy.sh [N=5] [PREFIX=maktub-warden-node]
#
# Idempotent: re-running re-deploys + re-stages secrets for each app.
set -euo pipefail

N="${1:-5}"
PREFIX="${2:-maktub-warden-node}"
FED_DIR="${FED_DIR:?set FED_DIR to the dealer output dir (contains shares/node-<i>.json + federation.json)}"
: "${BASE_SEPOLIA_RPC_URL:?set BASE_SEPOLIA_RPC_URL (read-only Base Sepolia RPC; treated as a secret)}"

# Resolve to an absolute FED_DIR before we cd into warden/ (build context).
FED_DIR="$(cd "$FED_DIR" && pwd)"
cd "$(dirname "${BASH_SOURCE[0]}")/../.."   # → warden/ : build context + the deploy/fly/* paths

command -v fly >/dev/null || { echo "flyctl not found — install + 'fly auth login' first" >&2; exit 1; }

urls=()
for i in $(seq 1 "$N"); do
  app="${PREFIX}-${i}"
  share="${FED_DIR}/shares/node-${i}.json"
  [ -f "$share" ] || { echo "missing share $share — run the dealer first (see header)" >&2; exit 1; }

  echo "==> ${app}"
  fly apps create "$app" --machines >/dev/null 2>&1 || true   # idempotent: ok if it exists

  # Secrets: encrypted at rest, injected as env. The share is base64'd to one line so it
  # survives as a single secret value. (Set on YOUR machine only — never committed/logged in CI.)
  fly secrets set -a "$app" --stage \
    WARDEN_SHARE_B64="$(base64 < "$share" | tr -d '\n')" \
    WARDEN_RPC_URL="$BASE_SEPOLIA_RPC_URL" >/dev/null

  fly deploy -a "$app" -c deploy/fly/fly.node.toml
  urls+=("https://${app}.fly.dev")
done

echo
echo "──────────────────────────────────────────────────────────────"
echo "Federation node endpoints (the NODES list clients poll for /partial):"
printf '%s\n' "${urls[@]}" | paste -sd, -
echo
echo "Public federation file (give clients this + the NODES list above): ${FED_DIR}/federation.json"
echo "Smoke a node:  curl https://${PREFIX}-1.fly.dev/health   # → {\"status\":\"ok\"}"
echo "⚠️ All-ours = ZERO timing-security (D-036). Testnet only until nodes are independent operators."
echo "──────────────────────────────────────────────────────────────"
