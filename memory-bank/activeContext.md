# EpicOrganizer — Active Context

## Current Phase

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

- Scaffold Tauri (`vanilla-ts`), fill `commands` in `worktree.config.json`, then build M2–M4.
