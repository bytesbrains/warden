#!/usr/bin/env node
// Local-devnet Warden test harness (#181 follow-up). Drives the full loop against a local
// Hardhat node + warden federation, on the FROZEN protocol (deterministic ids D-038, canonical
// Flash D-039, permissionless backstop D-040). Cases:
//   1.  sealed-before    active Beat → decrypt must FAIL
//   1b. discovery        getInboxBeats surfaces the new beat from canonical state (D-038)
//   2.  readable-after   evm_increaseTime → executor execute → decrypt SUCCEEDS (payload matches)
//   3.  revocation       deactivated Beat → decrypt must NEVER succeed
//   4.  backstop         past expiry+EXECUTION_GRACE, an UNSTAKED wallet executes → decrypt
//                        SUCCEEDS — delivery works with no executor (D-040, #222)
//
// Execution modes:
//   EXECUTE_MODE=self     (default) the harness stakes + calls execute() itself — fast,
//                         works with evm_increaseTime. Use this for local verification.
//   EXECUTE_MODE=external the real `executor` container fires; the harness time-travels and
//                         waits for `executed`. NOTE: the executor schedules on wall-clock
//                         time, so evm_increaseTime alone won't trigger it on a local chain —
//                         external mode needs real ≥INTERVAL elapse (≥1h, MIN_INTERVAL). It is
//                         the faithful topology for a real testnet, where wall-clock≈chain-time.
//
// Defaults target a fresh `npx hardhat node` + `scripts/deploy.js` (deterministic addresses).
import fs from "node:fs";
import { ethers } from "ethers";
import * as chain from "../lib/chain.mjs";
import * as warden from "../lib/warden.mjs";

const env = (k, d) => process.env[k] ?? d;
const CFG = {
  rpc: env("LOCAL_RPC_URL", "http://127.0.0.1:8545"),
  // Required: the Hardhat account-0 key (printed by `npx hardhat node`). Never committed.
  pk: env("PRIVATE_KEY"),
  core: env("LOCAL_MAKTUB_CORE", "0x5FC8d32690cc91D4c39d9d3abcBD16989F875707"),
  rewards: env("LOCAL_EXECUTOR_REWARDS", "0xDc64a140Aa3E981100a9becA4E685f962f0cF6C9"),
  token: env("LOCAL_MKTB_TOKEN", "0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512"), // gitleaks:allow (deterministic local devnet address)
  registry: env("LOCAL_RECIPIENT_REGISTRY", "0x5FbDB2315678afecb367f032d93F642f64180aa3"),
  federation: env("FEDERATION", "warden/fed/federation.json"),
  nodes: env("NODES", "http://127.0.0.1:8551"),
  bin: env("WARDEN_BIN", "warden/target/debug/warden"),
  store: env("STORE", "warden/e2e/local/store"),
  interval: Number(env("INTERVAL", "3600")),
  mode: env("EXECUTE_MODE", "self"),
};

const log = (m) => console.log(`\x1b[36m▶\x1b[0m ${m}`);
const pass = (m) => console.log(`\x1b[32m✓ ${m}\x1b[0m`);
const die = (m) => { console.error(`\x1b[31m✗ ${m}\x1b[0m`); process.exit(1); };
const sleep = (s) => new Promise((r) => setTimeout(r, s * 1000));

const ERC20 = ["function approve(address,uint256) returns (bool)"];
const REWARDS = [
  "function minimumStake() view returns (uint256)",
  "function isActiveExecutor(address) view returns (bool)",
  "function stake(uint256)",
];

async function stakeIfNeeded(signer, addr) {
  const r = new ethers.Contract(CFG.rewards, REWARDS, signer);
  if (await r.isActiveExecutor(addr)) return;
  const min = await r.minimumStake();
  await (await new ethers.Contract(CFG.token, ERC20, signer).approve(CFG.rewards, min)).wait(1);
  await (await r.stake(min)).wait(1);
  log(`staked ${ethers.formatEther(min)} MKTB → executor active`);
}

const timeTravel = (p, s) => p.send("evm_increaseTime", [s]).then(() => p.send("evm_mine", []));

// Seal a fresh payload to the next Beat, create it on-chain. Returns { beatId, cid, secret, message }.
// The Veil condition for a local Beat — chain 31337 (the CLI's --beat hardcodes 84532, so we
// build the JSON ourselves and use --condition). Matches the node's WARDEN_CHAIN_ID.
function writeCondition(beatId) {
  const cond = {
    type: "contract",
    chain: Number(env("LOCAL_CHAIN_ID", "31337")),
    address: CFG.core,
    fn: "getHeartbeat(uint256)",
    args: [String(beatId)],
    word: 7,
    test: { cmp: "==", value: true },
    meta: { finality: 1, tier: 1 },
  };
  fs.mkdirSync(CFG.store, { recursive: true });
  const path = `${CFG.store}/cond-${beatId}.json`;
  fs.writeFileSync(path, JSON.stringify(cond));
  return path;
}

async function sealAndCreate(core, registry, addr, label) {
  await chain.ensureRegisteredRecipient(registry, addr);
  // D-038: the id is keccak256(sender, salt), known BEFORE create — so we seal the Veil
  // condition to this beat's own id with no counter race, then create with the same salt.
  const salt = chain.randomSalt();
  const beatId = chain.beatId(addr, salt);
  const { public: pub, secret } = warden.keygen(CFG.bin);
  const message = `local-veil ${label} beat=${beatId}`;
  const cid = warden.encrypt(CFG.bin, {
    federation: CFG.federation, recipient: pub, conditionFile: writeCondition(beatId), message, store: CFG.store,
  });
  await chain.createBeat(core, salt, [addr], cid, CFG.interval);
  return { beatId, cid, secret, message };
}

function tryDecrypt(state, timeout) {
  return warden.decrypt(CFG.bin, {
    federation: CFG.federation, nodes: CFG.nodes, key: state.secret,
    envelope: state.cid, store: CFG.store, timeout, interval: 2,
  });
}
const mustSeal = (s) => { if (tryDecrypt(s, 6).ok) die("decrypted while it should be sealed!"); };

async function main() {
  if (!CFG.pk) die("set PRIVATE_KEY to the Hardhat account-0 key (printed by `npx hardhat node`)");
  const { provider, wallet } = chain.connect(CFG.rpc, CFG.pk);
  const addr = wallet.address;
  // NonceManager: serialize nonces so back-to-back txs (approve+stake, create×N) don't race.
  const signer = new ethers.NonceManager(wallet);
  const c = chain.contracts(signer, CFG);
  log(`local devnet ${CFG.rpc} (chain ${(await provider.getNetwork()).chainId}), mode=${CFG.mode}`);
  // Fail loudly if the (deterministic) MaktubCore address has no code — chain not deployed,
  // or deploy.js's order drifted the addresses. Override with LOCAL_MAKTUB_CORE.
  if ((await provider.getCode(CFG.core)) === "0x") {
    die(`no contract at MaktubCore ${CFG.core} — is the chain deployed? (override LOCAL_MAKTUB_CORE if deploy.js order changed)`);
  }

  // CASE 1 — sealed before execution.
  const s1 = await sealAndCreate(c.core, c.registry, addr, "trigger");
  mustSeal(s1);
  pass(`CASE 1 sealed-before: beat ${s1.beatId} undecryptable while active`);

  // CASE 1b — discovery (D-038): the recipient index must surface the new beat from canonical
  // state (the deterministic id is what was sealed), so a recipient can find it with no indexer.
  const inbox = await chain.inboxBeats(c.core, addr);
  if (!inbox.includes(s1.beatId)) die(`discovery: beat ${s1.beatId} not in getInboxBeats(${addr})`);
  pass(`CASE 1b discovery: getInboxBeats surfaces beat ${s1.beatId}`);

  // CASE 2 — readable after execution.
  await timeTravel(provider, CFG.interval + 60);
  if (CFG.mode === "self") {
    await stakeIfNeeded(signer, addr);
    await chain.executeBeat(c.core, s1.beatId);
    log(`executed beat ${s1.beatId} (self mode)`);
  } else {
    log(`waiting for the executor container to fire beat ${s1.beatId}…`);
    for (let i = 0; i < 60 && !(await chain.status(c.core, s1.beatId)).executed; i++) await sleep(2);
  }
  const r = tryDecrypt(s1, 60);
  if (!r.ok) die(`still sealed after execution: ${r.stderr.trim()}`);
  if (r.stdout.trim() !== s1.message) die(`payload mismatch: ${JSON.stringify(r.stdout)}`);
  pass(`CASE 2 readable-after: recovered payload matches after executed==true`);

  // CASE 3 — revocation.
  const s3 = await sealAndCreate(c.core, c.registry, addr, "revoke");
  await chain.deactivateBeat(c.core, s3.beatId);
  mustSeal(s3);
  pass(`CASE 3 revocation: deactivated beat ${s3.beatId} never decryptable`);

  // CASE 4 — permissionless backstop (#222/D-040): delivery must work even with NO executor.
  // A fresh, UNSTAKED wallet (not an executor) executes after expiry + EXECUTION_GRACE; the
  // federation releases on executed==true regardless of who triggered it, so the recipient
  // self-rescues. Proves delivery liveness is independent of the executor market.
  const s4 = await sealAndCreate(c.core, c.registry, addr, "backstop");
  const grace = Number(await c.core.EXECUTION_GRACE());
  await timeTravel(provider, CFG.interval + grace + 60); // past expiry + grace
  const rescuer = ethers.Wallet.createRandom().connect(provider);
  await (await signer.sendTransaction({ to: rescuer.address, value: ethers.parseEther("1") })).wait(1);
  if (await c.rewards.isActiveExecutor(rescuer.address)) die("rescuer unexpectedly an executor");
  await (await chain.contracts(rescuer, CFG).core.execute(s4.beatId)).wait(1);
  log(`executed beat ${s4.beatId} via backstop (unstaked ${rescuer.address.slice(0, 10)}…)`);
  const r4 = tryDecrypt(s4, 60);
  if (!r4.ok) die(`backstop: still sealed after execution: ${r4.stderr.trim()}`);
  if (r4.stdout.trim() !== s4.message) die(`backstop payload mismatch: ${JSON.stringify(r4.stdout)}`);
  pass(`CASE 4 backstop: delivery succeeded with NO executor (recipient self-rescued after grace)`);

  pass("LOCAL WARDEN LOOP VERIFIED.");
}
main().catch((e) => { if (e?.stack) console.error(e.stack); die(e?.message || String(e)); });
