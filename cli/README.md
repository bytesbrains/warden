# warden-cli (`warden`)

The Warden client (Phase 0 PoC). Seals payloads into the `warden-v1`
double-wrap, publishes them to a content-addressed store, and recovers them by polling the
federation for partials, combining, and opening.

> ⚠️ **Not audited. Not for production.** PoC — see [`../docs/05-threat-model.md`](../docs/05-threat-model.md).

## Commands

```bash
# 1. Recipient keypair (public → whoever seals to you; secret → you).
warden keygen --out heir.key            # secret written 0600; public printed

# 2. Seal a payload to a condition + recipient → prints a CID.
warden encrypt --federation fed/federation.json --recipient <pubkey-hex> \
  --beat 123 [--core 0x…]   `# Veil: MaktubCore.getHeartbeat(123).executed==true` \
  --message "the twelve words" [--payload file] [--store store] [--finality 32]
#   …or an arbitrary condition:  --condition cond.json   (instead of --beat)

# 3. Recover it — polls the federation until t nodes release, then combines + opens.
warden decrypt --federation fed/federation.json --nodes http://n1,http://n2,http://n3 \
  --key-file heir.key   `# or --key <hex>` \
  --envelope <cid>  [--store store] [--timeout 120] [--interval 3] [--out plain.txt]
```

`decrypt` is **idempotent / retry-until-released**: because the condition is monotonic, it
polls (every `--interval`s up to `--timeout`s) until `t` nodes release, verifying each partial
against its published share public key and deduping by node — so one bad node can't corrupt
the combine. Run it before the beat fires and it simply keeps trying; run it after and it
succeeds immediately.

## How it fits

- The double-wrap (`seal`/`open`), federation file format, and threshold `combine` all live in
  `warden-core`; this crate is the glue + key management + CID store + node polling.
- **CID store** (`store/<cid>.json`, `cid = sha256(envelope)`) stands in for IPFS/Arweave; the
  on-chain footprint of a real Beat is just this id. A real backend is a later phase.
- Recipient keys are secp256k1 (the ECIES recipient gate); a consuming app can align these with its own recipient registry (e.g. Maktub's).

## Test

`cargo test -p warden-cli` includes a fully offline end-to-end flow: it deals a federation,
stands up mock nodes that release partials, and drives `keygen → encrypt → decrypt` through
the real binary, asserting the payload round-trips. Live-chain release is covered by the e2e harness ([`../e2e/README.md`](../e2e/README.md)).
