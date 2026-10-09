# EpicOrganizer — M3 detect-encrypt history

## 2026-10-09 — M3 detect + encrypt flow (consent-gated)

- Task: `2026-10-09-m3-detect-encrypt` (branch `task/m3-detect-encrypt`). Goal: detect
  likely-confidential files with explainable reasons, encrypt approved files with the locked
  AES-256-GCM envelope (Argon2id passphrase, plaintext deleted), prove the decrypt round-trip,
  and add PDF/docx extraction deferred from M2.
- Built: `src-tauri/src/{extract,crypto,detect}.rs` + `commands.rs` / `lib.rs` / `tools.rs` /
  `agent.rs` updates. Extraction: text (64 KiB read), PDF (`pdf-extract`), docx (`zip` row +
  `quick-xml` `w:t`/`w:p`/entity handling), 256 KiB cap. Crypto: `EORG1 | salt | wrap-nonce |
  wrapped-DEK+tag | content-nonce | ciphertext+tag`, Argon2id v19 m=19456/t=2/p=1, AES-256-GCM,
  `Zeroizing` secrets, temp-file + rename + `sync_all`, plaintext delete best-effort, decrypt
  keeps the sealed copy. Detection: bounded 500-file/6-deep walk skipping symlinks and `.enc`;
  name rules + content rules (SSN, Luhn card, private key, AWS/API key, password assignment);
  one model call over ≤8×600-char numbered snippets with lenient JSON parsing; failure degrades
  to rules + warning. UI: auto-scan after `execute_plan`, reason badges + checkboxes, confirmed
  passphrase (session memory only), per-file results with Decrypt buttons.
- Files: `index.html`, `src/{organizer,types}.ts`, `src/styles.css`, `src-tauri/Cargo.{toml,lock}`,
  `src-tauri/src/{extract,crypto,detect,commands,lib,tools,agent}.rs`,
  `docs/adr/0001-confidential-detection-and-encryption.md`.
- Validation: `npm run build` exit 0; `cargo check` zero warnings; `cargo test` 28 passed/2
  ignored; live smoke on Ollama `llama3.2:3b` (organize passed; detection flagged
  `passwords.txt` with rule reasons + `notes.md` via model verdict); `tauri dev` window pid
  32216 title `EpicOrganizer`, port 1420 free. Release installers not rebuilt (outside gate).
- Follow-ups: llama3.2:3b detection false-positive (`notes.md`) — reasons are user-reviewed and
  rules stay authoritative, consider 8B for the demo; `commands.stop` still unset in
  `worktree.config.json`; ADR for the stack trio still not written (ADR 0001 covers detection +
  encryption only).
