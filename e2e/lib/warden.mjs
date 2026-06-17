// Thin wrapper around the `warden` client binary for the e2e harness.
import { spawnSync } from "node:child_process";

function run(bin, args) {
  const r = spawnSync(bin, args, { encoding: "utf8" });
  if (r.error) throw new Error(`failed to spawn ${bin}: ${r.error.message}`);
  return { ok: r.status === 0, stdout: r.stdout || "", stderr: r.stderr || "" };
}

// Pull `<key> <value>` from a `key value` output line.
function field(out, key) {
  const line = out.split("\n").find((l) => l.startsWith(`${key} `));
  if (!line) throw new Error(`no \`${key}\` line in output:\n${out}`);
  return line.slice(key.length + 1).trim();
}

export function keygen(bin) {
  const r = run(bin, ["keygen"]);
  if (!r.ok) throw new Error(`keygen failed: ${r.stderr}`);
  return { public: field(r.stdout, "public"), secret: field(r.stdout, "secret") };
}

// Seal a payload. Either pass {beat, core} (CLI builds the Veil condition for chain 84532)
// or {conditionFile} (an arbitrary condition JSON — needed for any other chain, e.g. local 31337).
export function encrypt(bin, { federation, recipient, beat, core, conditionFile, message, store }) {
  const condArgs = conditionFile
    ? ["--condition", conditionFile]
    : ["--beat", beat, "--core", core];
  const r = run(bin, [
    "encrypt", "--federation", federation, "--recipient", recipient,
    ...condArgs, "--message", message, "--store", store,
  ]);
  if (!r.ok) throw new Error(`encrypt failed: ${r.stderr}`);
  return field(r.stdout, "cid");
}

// Returns { ok, stdout, stderr }. Caller asserts ok (decryptable) or !ok (still sealed).
export function decrypt(bin, { federation, nodes, key, envelope, store, timeout, interval }) {
  return run(bin, [
    "decrypt", "--federation", federation, "--nodes", nodes, "--key", key,
    "--envelope", envelope, "--store", store,
    "--timeout", String(timeout), "--interval", String(interval ?? 5),
  ]);
}
