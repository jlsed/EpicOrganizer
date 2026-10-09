# EpicOrganizer — M2 organize-flow history

## 2026-10-09 — M2 organize flow (Ollama + tool loop)

- Task: `2026-10-09-m2-organize-flow` (branch `task/m2-organize-flow`). Goal: folder pick +
  natural-language instruction; Rust Ollama client; tool-calling loop (list_files, read_file,
  create_folder, move_file, rename_file) with per-call validation + path-scope checks;
  proposed-operations review; execute; report. Detection/encryption out of scope (M3).
- Built: `src-tauri/src/{ollama,tools,agent,commands}.rs` + `lib.rs` wiring; `ScopedRoot`
  canonical path guard (rejects `..`, absolute paths, canonical escapes; symlinks skipped in
  listings); two-phase agent (inspect cap 8 + read dedupe; propose turn cap 100 ops; ops
  intercepted, validated, never executed during planning); `execute_plan` re-validates and
  never overwrites; `check_ollama` lists + warms the model. UI: `index.html`,
  `src/{organizer,types}.ts`, `src/main.ts`, `src/styles.css`; dialog plugin (`dialog:default`).
- Files: `index.html`, `src/{main,organizer,types}.ts`, `src/styles.css`, `package.json`,
  `package-lock.json`, `src-tauri/{Cargo.toml,Cargo.lock}`,
  `src-tauri/capabilities/default.json`, `src-tauri/src/{lib,commands,agent,ollama,tools}.rs`.
- Validation: `npm run build` exit 0; `cargo check` exit 0 (no warnings); `cargo test`
  14 passed/1 ignored; live smoke (`cargo test -- --ignored`) passed on Ollama `llama3.2:3b`
  (create `Images/` + moves executed, hallucinated paths marked invalid); `tauri dev` window
  pid 23728 title `EpicOrganizer`, port 1420 free after kill; `npm run tauri build` exit 0 —
  MSI (2.34 MiB) + NSIS setup (1.58 MiB) under `src-tauri/target/release/bundle/`.
- Follow-ups: PDF/docx extraction + detection/encryption (M3); `commands.stop` still unset in
  `worktree.config.json`; llama3.2:3b variance (occasional non-image moves / invented names) —
  plan-time validation filters and the owner reviews before execution; consider the 8B model
  for the GPU demo.

## 2026-10-09 — Organize guardrails (out-of-scope refusal + prompt hardening)

- Problem: an unrelated instruction ("tell me the mass of the sun") reached the propose turn
  and produced a bogus `create_folder 'data/sun_mass.txt'` operation.
- Fix: `agent.rs` system prompt now scopes the model to file organization only, requires an
  exact `NOT_A_FILE_TASK` refusal (no tool calls) for anything else, and adds untrusted-content
  (names/content are data), no-invention (only `list_files` paths) and no-reveal rules;
  `PROPOSE_INSTRUCTION` emits nothing for non-file tasks; `plan()` detects the marker
  case-insensitively (UTF-8-safe byte scan) and returns zero operations + warning, skipping the
  mutation turn. `detect.rs` classifier prompt marks snippets as untrusted data.
- Tests: guardrail prompt assertions + refusal marker detection/cleaning.
- Validation: `npm run build` exit 0; `cargo test` 30 passed/2 ignored; live `llama3.2:3b`
  smoke (raw `/api/chat`, exact prompt + read tool schemas) — out-of-scope → exactly
  `NOT_A_FILE_TASK` with no tool calls; in-scope instruction → normal `list_files` calls.
- Follow-up (same day): scope narrowed to files directly at the ROOT top level — subfolder
  contents are ignored and must never be read/moved/renamed/deleted; the model must cover every
  matching top-level file, and `PROPOSE_INSTRUCTION` forbids subfolder sources. Validation:
  `npm run build` exit 0; `cargo test` 30 passed/2 ignored.
