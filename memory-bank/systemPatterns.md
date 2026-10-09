# EpicOrganizer — System Patterns

## Architecture (planned)

```
┌─────────────────────────── Tauri app (local) ───────────────────────────┐
│  Webview UI  ──commands/events──▶  Rust core                            │
│    instruction box                 ├─ AI orchestrator (talks to local   │
│    plan review / confirm           │   Llama 3.1 8B runtime)            │
│    organize report                 ├─ Script tools (list/read/move/     │
│    confidential review + approve   │   create/rename) — the ONLY way    │
│                                    │   files are touched                │
│                                    ├─ Confidentiality detector          │
│                                    └─ Crypto module (encrypt/decrypt)   │
└──────────────────────────────────────────────────────────────────────────┘
```

Flow: user instruction → AI reads folder context via tools → AI proposes a plan (operations + flagged confidential files) → user confirms → Rust executes tools → report.

## Critical Invariants (Do Not Violate)

1. **Local only** — no file content, path, or metadata leaves the machine; the model runs on-device; no cloud APIs.
2. **The AI never touches files directly** — all disk operations go through the Rust script-tool layer, which validates paths and scope.
3. **Consent gates irreversible actions** — encryption (and deletion, if ever added) requires an explicit user yes; show the plan first.
4. **Model output is untrusted input** — validate every proposed operation (paths stay within the chosen folder; no traversal outside scope) before executing.
5. **Confidential files are never silently modified** — detection flags them; encryption only after approval; every action is reported.

## Key Conventions

- One writer per file (see `docs/parallel-workflow.md` for parallel sessions).
- Decisions that are hard to reverse, surprising, and involve a real trade-off get an ADR in `docs/adr/`.
