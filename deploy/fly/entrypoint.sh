#!/bin/sh
# Materialize this node's secret share (injected as the WARDEN_SHARE_B64 fly secret) to a
# 0600 file, then exec wardend. The share never lands in the image or in env-as-plaintext-file
# beyond the ephemeral container FS; it is set via `fly secrets set` (encrypted at rest).
set -eu

: "${WARDEN_SHARE_B64:?set the per-node share as the WARDEN_SHARE_B64 fly secret (base64 of shares/node-<i>.json)}"
: "${WARDEN_RPC_URL:?set WARDEN_RPC_URL as a fly secret (read-only Base Sepolia RPC)}"

umask 077
echo "$WARDEN_SHARE_B64" | base64 -d > /tmp/share.json
export WARDEN_SHARE_FILE=/tmp/share.json

exec wardend
