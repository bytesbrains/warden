// Node round-trip test for warden-wasm against the Rust-generated fixture (cross-language
// validation). Build the pkg first:  wasm-pack build --target nodejs --out-dir pkg --release
// Then:  node warden/wasm/test/round_trip.cjs
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const wasm = require("../pkg/warden_wasm.js");

const fx = JSON.parse(fs.readFileSync(path.join(__dirname, "fixture.json"), "utf8"));
const condJson = JSON.stringify(fx.condition);
const fedJson = JSON.stringify(fx.federation);
const envJson = JSON.stringify(fx.gatedEnvelope);

// 1. condition_identity matches the Rust fixture AND the committed KAT (core/tests/vectors.rs).
assert.strictEqual(wasm.condition_identity(condJson), fx.identity, "identity vs fixture");
assert.strictEqual(
  fx.identity,
  "47fce3a147fc844978e8301a7aedbf437100eda9f769ac0d559c85d806cdb68e",
  "identity vs #207 KAT"
);

// 2. combine the fixture's partials → the fixture's d_id (wasm matches Rust).
assert.strictEqual(wasm.combine(JSON.stringify(fx.partials), fx.identity, fedJson), fx.dId, "combine");

// 3. open the fixture's gated envelope with d_id → the original blob, byte-for-byte.
assert.strictEqual(wasm.open_gated(envJson, fx.dId), fx.blob, "open_gated vs blob");

// 4. round-trip: wasm seals a NEW gate (fresh random obk), opens with the same d_id → same blob.
const newEnv = wasm.seal_gated(condJson, fx.masterPub, fx.network, fx.blob);
assert.strictEqual(wasm.open_gated(newEnv, fx.dId), fx.blob, "wasm seal→open round-trip");

// 5. combine is tolerant of a noisy federation — a malformed/extra partial is dropped, not fatal.
const noisy = [...fx.partials, "00".repeat(48)]; // a bogus 48-byte "partial"
assert.strictEqual(wasm.combine(JSON.stringify(noisy), fx.identity, fedJson), fx.dId, "combine tolerates noise");

console.log("✓ warden-wasm round-trip OK — identity, combine, open_gated, seal→open, noise-tolerant combine");
