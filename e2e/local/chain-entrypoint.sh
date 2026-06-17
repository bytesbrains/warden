#!/usr/bin/env bash
# Start a Hardhat node, wait for RPC, deploy contracts/v3, then stay up.
set -e

npx hardhat node --hostname 0.0.0.0 > /tmp/node.log 2>&1 &

echo "[chain] waiting for RPC…"
until node -e "fetch('http://127.0.0.1:8545',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({jsonrpc:'2.0',id:1,method:'eth_blockNumber',params:[]})}).then(r=>r.json()).then(()=>process.exit(0)).catch(()=>process.exit(1))" 2>/dev/null; do
  sleep 1
done

echo "[chain] deploying contracts/v3 (deterministic addresses)…"
npx hardhat run scripts/deploy.js --network localhost
echo "[chain] READY — MaktubCore at 0x5FC8d32690cc91D4c39d9d3abcBD16989F875707"

tail -f /tmp/node.log
