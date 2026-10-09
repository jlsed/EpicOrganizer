# EpicOrganizer — Tech Context

## Stack (locked 2026-10-09)

| Layer | Choice | Version | Notes |
| --- | --- | --- | --- |
| Shell / UI | Tauri (desktop) + `vanilla-ts` webview | TBD | no JS framework |
| Core backend | Rust | TBD | file ops, tool execution, orchestration glue |
| Crypto | AES-256-GCM + Argon2id (`aes-gcm`, `argon2`) | TBD | passphrase envelope; user enters a password to encrypt/decrypt |
| Content extraction | Rust crates (TBD: `pdf-extract`, `docx-rust`/`quick-xml`) | TBD | text files native; PDF + docx extracted, truncated before the model |
| Local AI (demo) | Llama 3.2 3B via Ollama | 3B Q4 | 2.0 GB disk, ~2.6 GB RAM loaded, ~7 tok/s on this machine (measured 2026-10-09) |
| Local AI (target) | Llama 3.1 8B | 8B | drop-in upgrade on GPU hardware; same Ollama runtime |
| Database | none expected | — | hackathon scope; settings/state only if needed |
| Testing | TBD | | |

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

## Tooling & commands

> TODO once the Tauri scaffold exists: fill `commands` in `worktree.config.json` (build/test/dev for Tauri).

## Constraints

- **1-day hackathon** (2026-10-09) — MVP scope only.
- Windows dev machine; demo must work offline.
- Llama 3.1 8B needs a capable GPU/RAM — inference speed is the demo risk.
- No versioning for now (no `version` block in `worktree.config.json`).

## Checklist

- [ ] Fill `commands` (`test` / `release` / `stop`) in `worktree.config.json` once the scaffold exists
- [ ] Write ADRs for the locked decisions (stack trio; encryption approach) once they are final
