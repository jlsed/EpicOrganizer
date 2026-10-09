# EpicOrganizer — Active Context

## Current Phase

- 2026-10-09 (hackathon day): product + stack decisions locked in the memory bank; no code yet. Next: answer the open questions below, then scaffold and build the MVP.

## Locked Decisions (2026-10-09)

- Desktop app: Tauri shell with a Rust core.
- Webview stack: `vanilla-ts` (no JS framework).
- Local AI runtime (measured 2026-10-09): Ollama at `localhost:11434`; demo model `llama3.2:3b` (2 GB, ~2.6 GB RAM, tool calls + confidential detection verified); Llama 3.1 8B stays the drop-in target on GPU hardware.
- Encryption: AES-256-GCM with a passphrase envelope (Argon2id derives a KEK that wraps a random per-file DEK); the user enters their password to encrypt and to decrypt.
- Tool calling: native Ollama function-calling loop; Rust validates every call; `encrypt_file` needs an explicit user confirmation.
- Natural-language instructions; the AI proposes operations and the user sees the plan.
- Script tools (Rust) are the only path to file operations; they also read file contents.
- Confidentiality detection during organizing; flagged files optionally encrypted after user confirmation — encryption implemented in Rust.
- Highlight/differentiator: local AI for privacy + file organization.

## Open Questions (owner to answer next)

1. MVP scope cut: which demo flow is must-have vs stretch (organize / detect / encrypt)?
2. Content-reading scope: text-ish files only (txt/md/csv/json/source) vs document formats (PDF/docx) — determines which confidential files can be content-analyzed.

## Next Step

- Decide the scope cut, scaffold Tauri (`vanilla-ts`), fill `commands` in `worktree.config.json`, and start building.
