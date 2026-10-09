# EpicOrganizer — Tech Context

## Stack (locked 2026-10-09)

| Layer | Choice | Version | Notes |
| --- | --- | --- | --- |
| Shell / UI | Tauri (desktop) | TBD | webview UI; frontend framework TBD |
| Core backend | Rust | TBD | file ops, tool execution, orchestration glue |
| Crypto | Rust (crate TBD) | TBD | encryption/decryption of flagged files |
| Local AI | Llama 3.1 8B | 8B | runs on-device; inference runtime TBD |
| Database | none expected | — | hackathon scope; settings/state only if needed |
| Testing | TBD | | |

## Open infra decisions (resolve before/while scaffolding)

- **Inference runtime**: how Llama 3.1 8B is served locally (Ollama vs llama.cpp vs Rust bindings) and how Rust/Tauri talks to it.
- **Frontend framework** inside the Tauri webview.
- **Encryption scheme + key management**: cipher choice (e.g. AES-GCM via a Rust crate) and where the key/password lives.
- **Tool-calling protocol**: how model output becomes validated file operations (structured function calls vs a parsed plan format).

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
