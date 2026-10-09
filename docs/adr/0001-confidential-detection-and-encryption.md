# ADR 0001 — Confidential detection and consent-gated encryption

- **Status:** Accepted
- **Date:** 2026-10-09
- **Task:** `2026-10-09-m3-detect-encrypt`
- **Deciders:** EpicOrganizer owner (pre-build decision set 2026-10-09) + M3 build session

## Context

M3 must detect likely-confidential files while organizing, explain each flag, and encrypt
approved files with the locked AES-256-GCM passphrase envelope — all offline. Two design
questions were not fully settled by the pre-build decisions:

1. How does detection work? The demo model (`llama3.2:3b`) can emit a JSON confidential verdict,
   but small models are nondeterministic and can miss obvious patterns; pure heuristics miss
   semantic context.
2. Where does encryption sit in the tool loop? `encrypt_file` appears in the architecture's tool
   list, but encryption is irreversible and needs a passphrase the model must never see.

## Decision

1. **Hybrid, explainable detection.** A deterministic rule pass always runs (file-name patterns
   such as `password`/`passport`/`ssn` plus content checks for SSNs, Luhn-valid card numbers,
   private-key blocks, AWS/API keys, and password assignments). Each hit records a human-readable
   reason. A single bounded local-model pass (≤8 files × 600-char snippets, numbered, strict JSON
   out) may add semantic findings; it never blocks the scan — if Ollama is down the report returns
   rule findings plus a warning.
2. **Encryption is user-driven, never a model tool.** Findings are surfaced in the UI with
   checkboxes; encryption runs only from an explicit "Encrypt selected" click plus a confirmed
   passphrase. The model has no `encrypt_file` tool and never sees the passphrase.
3. **Envelope format is fixed** as `EORG1 | salt(16) | wrap-nonce(12) | wrapped-DEK+tag(48) |
   content-nonce(12) | ciphertext+tag`. Argon2id (v19, m=19456 KiB, t=2, p=1) derives a 32-byte
   KEK from the passphrase and per-file salt; a random 256-bit DEK encrypts content and is wrapped
   by the KEK; AES-256-GCM throughout. Key material and the passphrase are zeroized.
4. **Asymmetric deletion policy.** Encryption writes `<name>.enc` durably, then deletes the
   plaintext best-effort (reported per file if deletion fails). Decryption writes the restored file
   and keeps the `.enc` copy — decryption is recovery, and the app never destroys the only sealed
   copy. A wrong passphrase is a clean GCM-tag failure with no partial output.

## Consequences

- Detection is deterministic enough to demo and explain, while the local model still adds semantic
  coverage; failures degrade instead of breaking the flow.
- The consent gate is enforced by construction: there is no code path where a model operation
  triggers encryption.
- Encrypted files are self-contained (no key file); losing the passphrase means losing the data.
  Files >256 MiB are refused to avoid OOM on the CPU-only demo machine.
- Decrypting leaves both the restored file and the `.enc` file; cleanup is manual.

## References

- `memory-bank/systemPatterns.md` — Encryption, Critical Invariants 3 & 5
- `memory-bank/techContext.md` — crypto and content-extraction rows
- `src-tauri/src/crypto.rs`, `src-tauri/src/detect.rs` — implementation
