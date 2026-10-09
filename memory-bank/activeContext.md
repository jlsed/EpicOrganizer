# EpicOrganizer — Active Context

## Current Phase

- 2026-10-09 (hackathon day): product + stack decisions locked in the memory bank; no code yet. Next: answer the open questions below, then scaffold and build the MVP.

## Locked Decisions (2026-10-09)

- Desktop app: Tauri shell with a Rust core.
- Local AI: Llama 3.1 8B orchestrates the organization (privacy = local inference).
- Natural-language instructions; the AI proposes operations and the user sees the plan.
- Script tools (Rust) are the only path to file operations; they also read file contents.
- Confidentiality detection during organizing; flagged files optionally encrypted after user confirmation — encryption implemented in Rust.
- Highlight/differentiator: local AI for privacy + file organization.

## Open Questions (owner to answer next)

1. Inference runtime: Ollama / llama.cpp / Rust crate bindings — and does the demo machine have the GPU/RAM for 8B?
2. Frontend framework for the Tauri webview (React / Svelte / vanilla)?
3. Encryption: algorithm + key handling (per-file password vs app key; where stored)?
4. Tool-calling: structured function calls from the model, or a parse-and-validate plan format?
5. MVP scope cut: which demo flow is must-have vs stretch (organize / detect / encrypt)?

## Next Step

- Grill the open questions, then write the MVP plan, fill `commands` in `worktree.config.json`, and start building.
