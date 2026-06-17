#!/usr/bin/env node
// Veil end-to-end harness on live Base Sepolia (Maktub #181 WS-E).
//
// Proves the whole loop against real chain state:
//   setup    create a Beat, seal a payload to `getHeartbeat(beatId).executed==true`,
//            assert it is UNDECRYPTABLE while the Beat is active.
//   trigger  executeHeartbeat (executor-only, after expiry), assert it becomes DECRYPTABLE
//            once `executed==true` is FINALIZED (nodes read at the `finalized` tag).
//   revoke   create + deactivate a second Beat, assert it is NEVER decryptable.
//   all      setup → wait out the interval → trigger → revoke (long-running; ≥ MIN_INTERVAL).
//
// Phases are resumable via a state file, so the ≥1h expiry wait need not hold a process open.
// Prereqs + finality/reorg notes: see ./README.md.
import fs from "node:fs";
import * as chain from "./lib/chain.mjs";
import * as warden from "./lib/warden.mjs";

const env = (k, d) => process.env[k] ?? d;
const CFG = {
  rpc: env("BASE_SEPOLIA_RPC_URL"),
  pk: env("PRIVATE_KEY"),
  core: env("CORE", "0xb603C96D089F64Ac487EE0bdaE97D49848F86133"),
  registry: env("REGISTRY", "0x49fbD4A3D67008766094Dc39B22DaAc77c2349Ff"),
  rewards: env("REWARDS", "0x86B5601DCf0C88481B3A146eee0b17aF8ba15A1F"),
  federation: env("FEDERATION", "warden/fed/federation.json"),
  nodes: env("NODES", "http://localhost:8531,http://localhost:8532,http://localhost:8533"),
  bin: env("WARDEN_BIN", "warden/target/debug/warden"),
  store: env("STORE", "warden/e2e/store"),
  interval: Number(env("INTERVAL", "3600")),
};
const STATE = `${CFG.store}/e2e-state.json`;
const SHORT = 20; // seconds — "should stay sealed" probe
const LONG = Number(env("DECRYPT_TIMEOUT", "1500")); // seconds — wait out finality lag

const log = (m) => console.log(`\x1b[36m▶\x1b[0m ${m}`);
const pass = (m) => console.log(`\x1b[32m✓ ${m}\x1b[0m`);
const die = (m) => { console.error(`\x1b[31m✗ ${m}\x1b[0m`); process.exit(1); };
function need(...keys) { for (const k of keys) if (!CFG[k]) die(`missing env for ${k}`); }
const loadState = () => JSON.parse(fs.readFileSync(STATE, "utf8"));
const saveState = (s) => { fs.mkdirSync(CFG.store, { recursive: true }); fs.writeFileSync(STATE, JSON.stringify(s, null, 2)); };

// Seal a fresh payload to a freshly-created Beat; return its state. Used by setup + revoke.
async function sealToNewBeat(core, rewards, registry, wallet, label) {
  await chain.ensureRegisteredRecipient(registry, wallet.address);
  const beatId = await chain.nextBeatId(core);
  const { public: pub, secret } = warden.keygen(CFG.bin);
  const message = `veil-e2e ${label} beat=${beatId} ts=${Date.now()}`;
  const cid = warden.encrypt(CFG.bin, {
    federation: CFG.federation, recipient: pub, beat: beatId, core: CFG.core, message, store: CFG.store,
  });
  log(`creating Beat ${beatId} (interval=${CFG.interval}s), payload CID ${cid.slice(0, 16)}…`);
  await chain.createBeat(core, [wallet.address], cid, CFG.interval);
  const st = await chain.status(core, beatId); // reverts if the predicted id is wrong
  if (st.executed || st.deactivated) die(`fresh beat ${beatId} already executed/deactivated?!`);
  return { beatId, cid, secret, message };
}

function probeSealed(state) {
  const r = warden.decrypt(CFG.bin, {
    federation: CFG.federation, nodes: CFG.nodes, key: state.secret,
    envelope: state.cid, store: CFG.store, timeout: SHORT, interval: 5,
  });
  if (r.ok) die("payload DECRYPTED while it should be sealed — confidentiality broken!");
  pass(`still sealed (decrypt declined within ${SHORT}s, as expected)`);
}

async function setup() {
  need("rpc", "pk");
  const { wallet } = chain.connect(CFG.rpc, CFG.pk);
  const c = chain.contracts(wallet, CFG);
  const state = await sealToNewBeat(c.core, c.rewards, c.registry, wallet, "trigger");
  saveState(state);
  log("asserting UNDECRYPTABLE before execution…");
  probeSealed(state);
  pass(`setup done — beat ${state.beatId} sealed. Wait ≥ interval, then: veil-e2e.mjs trigger`);
}

async function trigger() {
  need("rpc", "pk");
  const state = loadState();
  const { wallet } = chain.connect(CFG.rpc, CFG.pk);
  const c = chain.contracts(wallet, CFG);
  await chain.requireExecutor(c.rewards, wallet.address);
  log(`executing beat ${state.beatId}…`);
  await chain.executeBeat(c.core, state.beatId);
  log(`executed. Polling for FINALIZED release (up to ${LONG}s — finality lag)…`);
  const r = warden.decrypt(CFG.bin, {
    federation: CFG.federation, nodes: CFG.nodes, key: state.secret,
    envelope: state.cid, store: CFG.store, timeout: LONG, interval: 10,
  });
  if (!r.ok) die(`still sealed after execution + ${LONG}s: ${r.stderr.trim()}`);
  // trim: decrypt writes raw payload bytes; guard against any trailing newline from capture.
  if (r.stdout.trim() !== state.message) die(`decrypted payload mismatch: got ${JSON.stringify(r.stdout)}`);
  pass("DECRYPTABLE after execution — recovered payload matches. The loop holds.");
}

async function revoke() {
  need("rpc", "pk");
  const { wallet } = chain.connect(CFG.rpc, CFG.pk);
  const c = chain.contracts(wallet, CFG);
  const state = await sealToNewBeat(c.core, c.rewards, c.registry, wallet, "revoke");
  log(`deactivating beat ${state.beatId} (executed can now never become true)…`);
  await chain.deactivateBeat(c.core, state.beatId);
  log("asserting NEVER decryptable…");
  probeSealed(state);
  pass(`revocation holds — deactivated beat ${state.beatId} stays sealed forever`);
}

async function all() {
  await setup();
  const waitS = CFG.interval + 120;
  log(`sleeping ${waitS}s for the Beat to expire (then trigger)…`);
  await new Promise((r) => setTimeout(r, waitS * 1000));
  await trigger();
  await revoke();
  pass("FULL VEIL LOOP VERIFIED on Base Sepolia.");
}

const phase = process.argv[2] || "all";
const phases = { setup, trigger, revoke, all };
if (!phases[phase]) die(`unknown phase ${phase} (use: setup | trigger | revoke | all)`);
phases[phase]().catch((e) => {
  if (e?.stack) console.error(e.stack); // preserve the trace for reverts/timeouts/type errors
  die(e?.message || String(e));
});
