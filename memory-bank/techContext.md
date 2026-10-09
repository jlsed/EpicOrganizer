# EpicOrganizer — Tech Context

## Stack (locked 2026-10-09)

| Layer | Choice | Version | Notes |
| --- | --- | --- | --- |
| Shell / UI | Tauri (desktop) + `vanilla-ts` webview | 2.12 (cli 2.12.1 / api 2.12.2) | no JS framework; scaffolded 2026-10-09 |
| Core backend | Rust | 1.94 (rustc/cargo) | file ops, tool execution, orchestration glue |
| Local AI client | `reqwest` (no default features + `json`) | 0.13.5 | HTTP to Ollama `127.0.0.1:11434`; added in M2 |
| Native dialogs | `tauri-plugin-dialog` | 2.8.1 | folder picker; `dialog:default` capability; added in M2 |
| Crypto | AES-256-GCM + Argon2id (`aes-gcm`, `argon2`) | 0.11.1 / 0.6.0 | passphrase envelope; `zeroize` 1.9.1 + `getrandom` 0.4; added in M3 |
| Content extraction | `pdf-extract` 0.12.1 + `zip` 9.0.0 (deflate only) + `quick-xml` 0.42.0 | — | M3: text/PDF/docx; truncated (4 KiB) before the model; image-only PDFs yield no text |
| Local AI (demo) | Llama 3.2 3B via Ollama | 3B Q4 | 2.0 GB disk, ~2.6 GB RAM loaded, ~7 tok/s on this machine (measured 2026-10-09) |
| Local AI (target) | Llama 3.1 8B | 8B | drop-in upgrade on GPU hardware; same Ollama runtime |
| Database | none expected | — | hackathon scope; settings/state only if needed |
| Testing | Rust unit tests + ignored live smoke | — | `cargo test` (28 unit tests + 2 ignored live smokes); live smoke `cargo test -- --ignored` needs Ollama; configured gate = `npm run build` |

## Implementation decisions (locked 2026-10-09)

- **Inference runtime**: Ollama (local HTTP at `localhost:11434`); demo model `llama3.2:3b` (measured, see below); 8B is a drop-in upgrade. The app talks to it behind one Rust interface so the model stays swappable.
- **Webview stack**: `vanilla-ts` inside Tauri — no JS framework.
- **Encryption**: AES-256-GCM; passphrase envelope — the user enters a password, Argon2id derives a KEK that wraps a random per-file DEK. Format + flow in `systemPatterns.md`.
- **Tool calling**: native Ollama function-calling loop (`tools` + `tool_calls`), Rust-validated per call with a loop cap; `encrypt_file` requires user confirmation in the UI.
- **MVP scope**: organize | detect | encrypt — all three are must-have for the demo.
- **Content reading**: text files, PDF, docx (Rust extraction, truncated for the model).

## Measured (2026-10-09, this machine)

- `llama3.2:3b` Q4 via Ollama 0.40.1: 2.0 GB disk; ~2.6 GB RAM while loaded (CPU-only); ~6.8 tok/s; short JSON responses 8–14 s wall.
- Tool calling: emits valid structured `tool_calls` JSON (validated against a `move_file` schema). Needs a `list_files` tool + prompt discipline so it doesn't guess paths.
- Confidential detection: correct JSON verdict on an SSN/bank-account sample.
- Demo tip: keep the model resident (Ollama `keep_alive`) to avoid cold-load pauses.
- M2 loop (measured while building, `llama3.2:3b` resident): inspect cap 8 steps + repeated-read dedupe; propose turn cap 100 ops; `num_ctx` 4096, `num_predict` 384 (inspect) / 1024 (propose); 300 s per-request timeout. Live plan+execute ≈61–115 s per demo folder.
- `check_ollama` warms the model (`/api/generate` + `keep_alive` 30m) so the first plan does not pay cold load.
- M3 (measured while building, `llama3.2:3b` resident): detection scan of 3 files = one model call (≤8 files × 600-char snippets), rule reasons + model verdict, no warnings; crypto round-trip covered by unit tests (Argon2id v19 m=19456 KiB, t=2, p=1; AES-256-GCM; 256 MiB per-file cap).

## Tooling & commands

Configured in `worktree.config.json` (commit `fa8f0df`):

```json
"commands": {
  "test": ["npm", "run", "build"],
  "release": ["npm", "run", "tauri", "build"]
}
```

`test` must run before any cargo command (`tauri::generate_context!` requires `dist/`). `stop` is not configured yet — add `["taskkill", "/F", "/IM", "epicorganizer.exe", "/T"]` so teardown can stop a locked dev app. `cargo check` and `cargo test` are task-acceptance/manual checks, not part of the gate.

## Constraints

- **1-day hackathon** (2026-10-09) — MVP scope only.
- Windows dev machine; demo must work offline.
- Llama 3.1 8B needs a capable GPU/RAM — inference speed is the demo risk.
- No versioning for now (no `version` block in `worktree.config.json`).

## Checklist

- [ ] Fill `commands` (`test` / `release` / `stop`) in `worktree.config.json` once the scaffold exists — partial, verified 2026-10-09: `test`/`release` configured in `fa8f0df`; `stop` still missing.
- [ ] Write ADRs for the locked decisions (stack trio; encryption approach) once they are final — partial, verified 2026-10-09 (M3): `docs/adr/0001-confidential-detection-and-encryption.md` exists (detection + encryption decision); stack-trio ADR still missing.
