# EpicOrganizer

A desktop file organizer driven by a fully local AI. Point it at a cluttered folder, describe how you want it organized in plain language, review the plan it proposes, and let it execute — while it detects likely-confidential files and seals the ones you approve with AES-256-GCM encryption. No cloud calls: airplane mode is the privacy proof.

Built during a 1-day hackathon (2026-10-09).

## Why

Files pile up and manual organizing is tedious. Existing AI organizers ship file names and contents to the cloud — a non-starter for tax returns, passports, and password files. EpicOrganizer runs the model on your machine (via [Ollama](https://ollama.com)) and keeps every byte of file data on disk.

## Features

- **Natural-language organization** — "Move all PDF invoices and receipts into an Invoices folder" becomes a plan you review, then an executed reality.
- **Plan before execution** — the AI proposes operations first; the UI renders create/move/rename descriptors and nothing touches disk until you click Execute.
- **Bounded tool layer** — the model can only call `list_files`, `read_file`, `create_folder`, `move_file`, and `rename_file`. Rust validates every call, every path stays inside the chosen root, and the AI never touches the filesystem directly.
- **Content-aware** — text, PDF, and docx files are extracted in Rust (truncated to 4 KiB) so categorization and detection can read content, not just file names.
- **Explainable confidential-file detection** — deterministic rules (SSNs, Luhn-valid card numbers, private-key blocks, AWS/API keys, password patterns, sensitive name patterns) plus one bounded local-model pass. Every flag carries a human-readable reason.
- **Consent-gated encryption** — flagged files can be sealed with AES-256-GCM under a passphrase envelope. Encryption is deliberately *not* a model tool: it needs an explicit user click plus a confirmed passphrase that is never stored.
- **Cross-session recovery** — a history rail lists previously organized folders and their sealed files; decrypting after an app restart restores the original file.
- **Refuses non-file requests** — ask it the mass of the sun and it declines with `NOT_A_FILE_TASK` instead of inventing operations.
- **100% local** — inference, detection, extraction, and crypto all run on-device.

## How it works

```
┌─────────────────────────── Tauri app (local) ───────────────────────────┐
│  Webview UI  ──commands/events──▶  Rust core                            │
│    instruction box                 ├─ AI orchestrator (Ollama + local   │
│    plan review / confirm           │   model; llama3.1:8b default)       │
│    organize report                 ├─ Script tools (list/read/move/     │
│    confidential review + approve   │   create/rename) — the ONLY way    │
│                                    │   files are touched                │
│                                    ├─ Confidentiality detector          │
│                                    └─ Crypto module (AES-256-GCM)       │
└──────────────────────────────────────────────────────────────────────────┘
```

Flow: pick a folder → write the instruction → the model inspects via read-only tools (capped) → a propose turn emits mutation tool calls that are intercepted, never executed → you review the plan → Rust re-validates and executes every operation → per-op report → automatic confidential scan → optional, consent-gated encryption.

Organization scope is the root's top level only: subfolder contents are ignored and never read, moved, renamed, or deleted.

## Privacy and security model

1. **Local only** — no file content, path, or metadata leaves the machine; the model runs on-device; there are no cloud APIs.
2. **The AI never touches files directly** — all disk operations go through the Rust tool layer, which validates paths and scope.
3. **Consent gates irreversible actions** — encryption requires an explicit yes and the plan is always shown first.
4. **Model output is untrusted input** — file names and contents are treated as data, never instructions; every proposed operation is validated before execution.
5. **Confidential files are never silently modified** — detection only flags them; every action is reported.

### Encryption envelope

```
EORG1 | salt | wrap-nonce | wrapped-DEK + tag | content-nonce | ciphertext + tag
```

Your passphrase goes through Argon2id (per-file salt) to derive a KEK, which wraps a random per-file DEK; all content encryption uses AES-256-GCM with random 96-bit nonces. Sealed files are stored as `<name>.enc`, the plaintext is deleted after a successful seal (best-effort), and decrypting restores the original name while keeping the sealed copy. A wrong passphrase fails the GCM tag cleanly — nothing is corrupted. See `docs/adr/0001-confidential-detection-and-encryption.md` and `docs/adr/0002-history-sidebar-cross-session-decrypt.md`.

## Getting started

### Prerequisites

- Windows (development and demo target), Node 22+, Rust 1.94+ (MSVC toolchain)
- [Ollama](https://ollama.com) running locally, with the default model pulled:

```powershell
ollama pull llama3.1:8b
```

`llama3.1:8b` is the default model (~4.9 GB on disk; a capable GPU is recommended for responsive planning). On CPU-only machines, set `EPICORGANIZER_MODEL=llama3.2:3b` to fall back to the smaller CPU-friendly model — the app talks to one Rust interface, so the model stays swappable.

### Run in development

```powershell
npm install
npm run tauri dev
```

### Build installers

```powershell
npm run tauri build
```

Produces MSI and NSIS installers under `src-tauri/target/release/bundle/`.

### Tests

The frontend build must precede any cargo command — `tauri::generate_context!` validates `frontendDist` at compile time:

```powershell
npm run build
cargo test --manifest-path src-tauri/Cargo.toml                 # 38 unit tests, 2 live smokes ignored
cargo test --manifest-path src-tauri/Cargo.toml -- --ignored    # live smokes; require Ollama
```

## Project layout

```
src/                     vanilla-ts webview UI (no JS framework)
  main.ts / organizer.ts    organize flow, plan review, report
  history.ts                history rail + sealed-file decrypt
src-tauri/src/           Rust core
  ollama.rs                local model client (chat + native tool calls)
  agent.rs                 inspect/propose orchestration + guardrails
  tools.rs                 script-tool schemas + path-scoped validation
  commands.rs              Tauri IPC surface
  extract.rs               text / PDF / docx content extraction
  detect.rs                explainable confidential-file detection
  crypto.rs                EORG1 envelope encrypt/decrypt
  history.rs               per-folder history store (app-data JSON)
docs/adr/                architecture decision records
memory-bank/             project brief, product context, patterns, progress
```

IPC commands: `list_folder_contents`, `plan_organize`, `cancel_plan`, `execute_plan`, `check_ollama`, `detect_confidential`, `encrypt_files`, `decrypt_file`, `list_history`, `remove_history`, `clear_history`.

History lives at `%APPDATA%\com.epicorganizer.app\history.json` — the last 50 organized folders with their last instruction, operation counts, and sealed-file state. It is written atomically and never stores passphrases.

## Status

Hackathon MVP (organize + detect + encrypt) is complete, plus the history sidebar with cross-session decrypt. Remaining demo polish is tracked in `memory-bank/progress.md`.

## Documentation

- `memory-bank/` — project brief, product context, system patterns, tech context, progress
- `docs/adr/` — decision records (detection/encryption, history/decrypt)
- `docs/parallel-workflow.md` — the parallel AI worktree workflow used to build this project
