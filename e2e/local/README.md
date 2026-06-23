# Local-devnet Warden harness

Proves the full Warden loop against a **local Hardhat chain** (no testnet funds, no ≥1h wait —
`evm_increaseTime` skips the timer), with **no changes to the warden code**. This is
the "prove Warden works for all conditions locally" gate. It drives Maktub's Veil layer as the
reference consumer, against a deployed Maktub protocol stack.

## What it verifies (`run.mjs`)

Runs against the consumer's **frozen protocol** (deterministic beat ids, canonical Flash state, permissionless backstop). The beat id is `keccak256(sender, salt)`, computed before create — so the condition is sealed to the beat's own id with no counter race.

1. **sealed-before** — payload sealed to `getHeartbeat(beatId).executed==true` is undecryptable while the Beat is active.
1b. **discovery** — `getInboxBeats(recipient)` surfaces the new beat from canonical state, so a recipient can find it with no indexer.
2. **readable-after** — `evm_increaseTime` → `execute(beatId)` (staked executor) → once `executed==true`, the node releases, the client combines + opens, and **the payload matches**.
3. **revocation** — a `deactivate`d Beat is never decryptable.
4. **backstop** — past `expiry + EXECUTION_GRACE`, an **unstaked** wallet executes and delivery still succeeds — proving liveness is independent of the executor market.

The crypto/threshold mechanics (t-of-n combine, partial verification, AEAD tamper, wrong-key)
are already covered offline by `warden/cli/tests/cli_flow.rs`; this harness adds the **live
on-chain integration** — the condition-watcher actually reading the consumer's contract (here `MaktubCore`) and gating on it.

## Run it (host mode — verified working)

```bash
# 1. Local chain + deployed stack (deterministic addresses)
npx hardhat node &                                   # :8545, chainId 31337
npx hardhat run scripts/deploy.js --network localhost

# 2. Warden side
cargo build -p warden-node -p warden-cli -p warden-dealer
cargo run -p warden-dealer -- --out warden/e2e/local/fed -n 1 -t 1 --network warden-local
WARDEN_SHARE_FILE=warden/e2e/local/fed/shares/node-1.json WARDEN_RPC_URL=http://127.0.0.1:8545 \
  WARDEN_CHAIN_ID=31337 WARDEN_FINALITY_TAG=latest WARDEN_LISTEN=127.0.0.1:8551 \
  warden/target/debug/wardend &

# 3. Drive the loop (from repo root — ethers resolves there). PRIVATE_KEY = the Hardhat
#    account-0 key that `npx hardhat node` prints on startup (public devnet key; never commit it).
export PRIVATE_KEY=<hardhat account-0 key from `npx hardhat node`>
FEDERATION=warden/e2e/local/fed/federation.json NODES=http://127.0.0.1:8551 \
  WARDEN_BIN=warden/target/debug/warden node warden/e2e/local/run.mjs
# → ✓ CASE 1 / 1b / 2 / 3 / 4 / ✓ LOCAL WARDEN LOOP VERIFIED.
```

For a real `t`-of-`n`, deal `-n 3 -t 2`, run three `wardend` on `:8551–8553`, and set
`NODES=http://127.0.0.1:8551,…8552,…8553`.

## Notes / gotchas (no code changes needed)

- **Chain id**: the CLI's `--beat` convenience hardcodes Base Sepolia (84532), so for the local
  chain the harness writes its own condition JSON (`chain: 31337`) and uses `--condition`. Same
  envelope, different chain.
- **Finality**: a local instamine chain has no real finality, so nodes run with
  `WARDEN_FINALITY_TAG=latest` (the node warns it's not reorg-safe — correct for local). The
  finality/reorg *property* itself can only be exercised on Base Sepolia (`warden/e2e/README.md`).
- **`EXECUTE_MODE`**: `self` (default) — the harness stakes + calls `execute()`; works with
  `evm_increaseTime`, so it's the fast local path. `external` — the real `executor` container
  fires. **Caveat:** the executor schedules on **wall-clock** time, so `evm_increaseTime` (chain
  time only) won't trigger it on a local chain — external mode needs real ≥`INTERVAL` elapse
  (≥1h). It is the faithful topology for a real testnet (wall-clock ≈ chain-time). For local,
  use `self`.

## Docker-compose (verified)

`docker compose -f docker-compose.yml up --build` brings up `chain` (Hardhat + auto-deploy) +
`dealer` + 3 `wardend` + the real `executor` (self-stakes via `executor/src/config.js`'s
`localhost` network). **Verified:** the full 2-of-3 loop (all three cases) runs green via the
host harness against the dockerized chain + nodes; the executor container connects, self-stakes,
becomes ACTIVE, and tracks the Beat. Autonomous firing from the container is wall-clock-bound on
a local chain (see the `EXECUTE_MODE` caveat) — verified on a real testnet instead.

```bash
export PRIVATE_KEY=<hardhat account-0 key from `npx hardhat node`>   # public devnet key; never committed
docker compose -f warden/e2e/local/docker-compose.yml up --build -d
FEDERATION=warden/e2e/local/_state/fed/federation.json \
  NODES=http://localhost:8551,http://localhost:8552,http://localhost:8553 \
  WARDEN_BIN=warden/target/debug/warden node warden/e2e/local/run.mjs
docker compose -f warden/e2e/local/docker-compose.yml down
```
