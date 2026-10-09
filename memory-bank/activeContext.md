# EpicOrganizer — Active Context

## Current Phase

- 2026-10-09 — M2 organize flow landed (task `2026-10-09-m2-organize-flow`, branch `task/m2-organize-flow`): four-stage UI (folder → instruction → review → report), Rust modules `ollama.rs` / `tools.rs` / `agent.rs` / `commands.rs`, dialog plugin + `dialog:default` capability, commands `plan_organize` / `execute_plan` / `check_ollama`. Deps added: `tauri-plugin-dialog` 2.8.1, `reqwest` 0.13.5 (json, no TLS), dev `tempfile` 3.27.0; npm `@tauri-apps/plugin-dialog`. Live smoke on Ollama `llama3.2:3b` passed (plan → approve → execute), dev window smoke passed, and `npm run tauri build` produced MSI + NSIS installers. Next: M3 (detect + encrypt; PDF/docx extraction lands here).
- 2026-10-09 — Tauri scaffold landed (task `2026-10-09-scaffold-tauri-app`, branch `task/scaffold-tauri-app`): vanilla-ts + Tauri v2 (`com.epicorganizer.app`), placeholder shell wired to Rust via `greet`; `npm run build`, `cargo check`, and a `tauri dev` window check all pass. Resolved deps: @tauri-apps/api 2.12.2, @tauri-apps/cli 2.12.1, tauri 2.12.2, tauri-build 2.7.1, vite 8.3.4, typescript 6.0.3 (node 22.14.0, rustc 1.94.0). Next: owner fills `commands` in `worktree.config.json` on main after review, then M2 (organize flow) starts from this scaffold.
- 2026-10-09 (hackathon day): all pre-build decisions locked (product, stack, runtime, webview, crypto, protocol, scope, content reading). No code yet. Next: scaffold Tauri and build.

## Locked Decisions (2026-10-09)

- Desktop app: Tauri shell with a Rust core.
- Webview stack: `vanilla-ts` (no JS framework).
- Local AI runtime (measured 2026-10-09): Ollama at `localhost:11434`; demo model `llama3.2:3b` (2 GB, ~2.6 GB RAM, tool calls + confidential detection verified); Llama 3.1 8B stays the drop-in target on GPU hardware.
- Encryption: AES-256-GCM with a passphrase envelope (Argon2id derives a KEK that wraps a random per-file DEK); the user enters their password to encrypt and to decrypt.
- Tool calling: native Ollama function-calling loop; Rust validates every call; `encrypt_file` needs an explicit user confirmation.
- MVP scope (all must-have): organize | detect confidential | encrypt-on-approval.
- Content reading: text files, PDF, and docx — text extracted in Rust and truncated before reaching the model; image-only/scanned PDFs yield no text (accepted limitation).
- Natural-language instructions; the AI proposes operations and the user sees the plan.
- Script tools (Rust) are the only path to file operations; they also read file contents.
- Confidentiality detection during organizing; flagged files optionally encrypted after user confirmation — encryption implemented in Rust.
- Highlight/differentiator: local AI for privacy + file organization.

## Open Questions

- None — the pre-build decision set is complete. New questions surface during scaffolding/build.

## Next Step

- Add `stop` to `commands` in `worktree.config.json` (`test`/`release` configured in `fa8f0df`), then build M3 (detect + encrypt; PDF/docx extraction) and M4 (demo polish / offline proof).

## Learnings & Preferences

- `create-tauri-app` 4.7.4 vanilla-ts emits no `public/` dir — static assets live under `src/assets/`.
- Dropping the template's unused `tauri-plugin-opener` requires removing all four touchpoints together (Cargo.toml dep, `.plugin()` call, `opener:default` capability, npm dep); a leftover permission fails at runtime.
- `tauri::generate_context!` validates `frontendDist` at compile time → run the frontend build before `cargo check` (the proposed `commands.test` reflects this ordering).
- `cargo check` does not link; the `tauri dev` smoke test is the first real link + window check. Cold timings on this machine: check ≈2m40s, dev link ≈2m14s.
- Dev smoke recipe: `Get-Process epicorganizer` + `MainWindowTitle`, then `taskkill /PID <npm-pid> /T /F`; verify port 1420 is free afterwards.
- `llama3.2:3b` generates unbounded text unless capped — set `options.num_predict` per phase (inspect 384 / propose 1024); one turn ran 1,920 tokens and hit the 300 s client timeout.
- Verify Ollama residency via `GET /api/ps` / the `llama-server` process; `check_ollama` warms the model with an empty `/api/generate` + `keep_alive` (30m).
- Windows `canonicalize` returns `\\?\` paths — canonicalize both sides before `starts_with`; skip symlinks in directory listings so they cannot expose content outside the root.
- `cargo test` also compiles `tauri::generate_context!`, so `npm run build` must precede *any* cargo command, not just `cargo check`.
- llama3.2:3b tool discipline is weak ("only images" not always obeyed; invents file names) — plan-time validation marks nonexistent targets invalid and the user reviews before execution; consider the 8B model for the GPU demo.
