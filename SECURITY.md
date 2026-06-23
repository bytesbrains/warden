# Security Policy

Warden is threshold cryptography for conditional decryption. Getting it right
matters — a flaw can mean a payload is readable *before* its condition holds, or
can never be read at all. Responsible disclosure is genuinely appreciated.

> **Phase 0 — proof of concept.** Warden is **not yet audited**, and the current
> federation is run entirely by its authors, so it provides **no security
> guarantee**. Do not protect real secrets with it today. An independent-operator
> federation, an external audit, and a public testnet are on the roadmap
> (see [`docs/07-roadmap.md`](docs/07-roadmap.md)).

## Reporting a vulnerability

**Please do not open a public issue for security reports.**

Report privately via GitHub's
["Report a vulnerability"](https://github.com/bytesbrains/warden/security/advisories/new)
button (the repository's *Security → Advisories* tab), or email
**contact@bytesbrains.com**. We aim to acknowledge within 72 hours.

## Scope

**In scope** — the cryptography and the release logic:
- `core/` — threshold IBE, the `warden-v1` double-wrap envelope, the federation file format.
- `node/` — the condition-watcher and threshold partial release, especially the evaluator (`node/src/eval.rs`) and finality handling.
- `cli/`, `ffi/`, `wasm/` — the client-side seal / combine / open paths.

**Out of scope** (intended and documented — not findings):
- The all-ours proof-of-concept federation having "no security" — see the threat model, [`docs/05-threat-model.md`](docs/05-threat-model.md).
- The trusted-dealer ceremony (`dealer/`) being a single point of trust — it's testnet-only; mainnet uses DKG.
- Denial of service against a single PoC node.

## What we especially want to hear about
- Any way a **non-recipient can recover plaintext** — the inner ECIES layer is meant to keep content unreadable even to a fully-colluding federation.
- Any way a threshold of honest nodes can be made to **release partials before the condition truly holds** on finalized chain state (beyond the documented reorg / RPC-spoofing model).
- Any way a sealed payload can be made **permanently unrecoverable** once its condition *does* hold.

## Disclosure

Warden is a public good; there is no paid bounty. We credit reporters in the
advisory and the fix notes unless you prefer to remain anonymous, and we ask for
coordinated disclosure — a reasonable window to ship a fix before public detail.
