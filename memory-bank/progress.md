# EpicOrganizer — Progress

## Current Status

- **2026-10-09 — M3 detect + encrypt flow on `task/m3-detect-encrypt`.** New Rust modules: `extract.rs` (text + PDF via `pdf-extract` 0.12 + docx via `zip` 9/`quick-xml` 0.42; 4 KiB model truncation, best-effort errors), `crypto.rs` (`EORG1` envelope — Argon2id per-file salt→KEK, random DEK wrapped by KEK, AES-256-GCM; encrypt writes `<name>.enc` durably then deletes plaintext best-effort and reports failures; decrypt restores the original name and keeps the `.enc`; `Zeroizing` passphrase/key material; 256 MiB cap), `detect.rs` (always-on explainable name/content rules — US SSN, Luhn-valid card, private-key block, AWS/API keys, password assignments — plus one bounded local-model JSON verdict pass that degrades to rules + warning when Ollama is unavailable). Commands `detect_confidential` / `encrypt_files` / `decrypt_file`; `read_file` now serves PDF/docx; UI step 5 auto-scans after execute, shows reason badges with checkboxes, collects a confirmed passphrase once per session, reports per-file encryption results and decrypts to prove recovery. ADR 0001 written. Validation: `npm run build` exit 0; `cargo check` zero warnings; `cargo test` 28 passed/2 ignored; live smoke on Ollama `llama3.2:3b` passed (organize still works; detection flagged `passwords.txt` with rule + model reasons); `tauri dev` window pid 32216 title `EpicOrganizer`, port 1420 free. Everything stays local. Performance: Argon2id (m=19456 KiB, t=2, p=1) tens of ms/file; detection adds one model call (≤8 files × 600-char snippets); organize timing unchanged (≈61–115 s typical).
- **2026-10-09 — M2 organize flow on `task/m2-organize-flow`.** Folder pick (dialog plugin) + natural-language instruction → Rust Ollama client (reqwest, `/api/chat`, tools, `keep_alive:30m`) → two-phase loop: read-only inspect (cap 8 steps, repeated-read dedupe) then a propose turn whose mutation tool calls are intercepted and validated by a canonical `ScopedRoot` path guard (nothing touches disk during planning); UI shows the plan (create/move/rename descriptors, invalid ones marked with reason) → approve executes via `execute_plan` (re-validates every op, never overwrites) → per-op report. Scope: PDF/docx extraction deferred to M3 (text-like files only). Validation: `npm run build` exit 0; `cargo test` 14 passed/1 ignored; `cargo check` exit 0; live plan→execute smoke on Ollama `llama3.2:3b` passed; `tauri dev` window smoke passed; `npm run tauri build` produced MSI + NSIS installers. Performance: plan+execute ≈61–115 s on the CPU-only demo machine; generations capped per phase; model warmed and kept resident.
- **2026-10-09 — Scaffold Tauri app on `task/scaffold-tauri-app`.** Official `create-tauri-app` 4.7.4 vanilla-ts template (Tauri v2, npm, identifier `com.epicorganizer.app`), branded to EpicOrganizer (1000×700) with a minimal placeholder shell that calls the Rust `greet` command over IPC; unused opener plugin removed. Validation (`worktree.config.json` commands still empty): `npm run build` exit 0 (tsc 6.0.3 + vite 8.3.4), `cargo check` exit 0 (tauri 2.12.2, 2m40s), `npm run tauri dev` opened the window (pid 27880, title `EpicOrganizer`). Performance: n/a (scaffold).
- **2026-10-09 — Memory bank initialized (hackathon day).** Product brief, product context, stack (Tauri + Rust + Llama 3.1 8B), and architecture invariants locked. No application code. Validation: n/a (docs only).

## Milestones

| Milestone | Status | Evidence |
| --- | --- | --- |
| M0 — Scaffold | done | rules + skills + memory bank present (commit `dfc583d`) |
| M1 — Decisions locked | done | `techContext.md`, `systemPatterns.md`, `activeContext.md` (2026-10-09) — product, stack, runtime, webview, crypto, tool protocol |
| M2 — Organize flow (instruction → folders) | done | `task/m2-organize-flow` — dialog folder pick + two-phase Ollama loop + review/execute UI; live smoke passed, `cargo test` 14 passed |
| M3 — Detect + encrypt flow (consent-gated) | done | `task/m3-detect-encrypt` — hybrid explainable detection + locked `EORG1` envelope encrypt/decrypt + PDF/docx extraction; `cargo test` 28 passed/2 ignored, live smoke passed |
| M4 — Demo polish / offline proof (MVP = M2+M3) | pending | — |

## Remaining Work

- Add `stop` to `commands` in `worktree.config.json` (`test`/`release` configured in `fa8f0df`); details in `techContext.md` → Tooling & commands.
- Build M4 (demo polish / offline proof); M3 shipped on `task/m3-detect-encrypt`. Check items off only with concrete evidence (AGENTS.md Rule 3).

> Validation evidence goes here per slice: commands run + results. Checklist items are only
> checked with concrete evidence (AGENTS.md Rule 3).
