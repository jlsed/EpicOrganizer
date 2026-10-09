# EpicOrganizer — System Patterns

## Architecture (planned)

```
┌─────────────────────────── Tauri app (local) ───────────────────────────┐
│  Webview UI  ──commands/events──▶  Rust core                            │
│    instruction box                 ├─ AI orchestrator (Ollama + local   │
│    plan review / confirm           │   model; llama3.2:3b demo)         │
│    organize report                 ├─ Script tools (list/read/move/     │
│    confidential review + approve   │   create/rename/encrypt) — the     │
│                                    │   ONLY way files are touched       │
│                                    ├─ Confidentiality detector          │
│                                    └─ Crypto module (AES-256-GCM)       │
└──────────────────────────────────────────────────────────────────────────┘
```

Flow: user instruction → AI loops via tool calls (list/read) → each call validated by Rust → AI proposes operations (flagged confidential files surfaced) → user confirms (encryption gated) → Rust executes → report.

### Orchestration loop (native tool calling)

- Tools: `list_files`, `read_file` (truncated), `create_folder`, `move_file`, `rename_file`, `encrypt_file`.
- Loop: model emits `tool_calls` → Rust validates (schema + canonicalized path stays under the chosen root + tool allowlist) → executes → returns results as tool messages → repeat; cap ≈20 iterations, then the model summarizes.
- M2 implementation: inspect phase runs read-only tools (cap 8 steps, repeated reads deduped) and a single propose turn emits mutation `tool_calls` that are intercepted and validated — nothing executes until the user approves; `execute_plan` re-validates every op on the way in. Generation is capped per phase (`num_predict` 384 inspect / 1024 propose).
- M3 implementation: `read_file` extracts text/PDF/docx (4 KiB to the model); detection is a post-execute hybrid scan (deterministic explainable rules + one bounded local-model JSON pass, rules-only fallback with a warning); encryption is user-driven from the findings panel — `encrypt_file` is deliberately NOT a model tool (consent + passphrase gate it, ADR 0001); decryption restores the file and keeps the sealed copy.
- `encrypt_file` never auto-runs: the UI shows the pending list and requires an explicit yes; the passphrase is collected once per session (never logged, zeroized after use).
- File content sent to the model is truncated (e.g. first ~4 KB) to fit the model's 4k context.

### Encryption (envelope, AES-256-GCM)

- User passphrase → Argon2id (per-file salt, 16 B) → KEK.
- Per file: random 256-bit DEK; content encrypted with the DEK; the DEK is wrapped with the KEK; AES-256-GCM with random 96-bit nonces throughout.
- File format: `"EORG1" | salt | wrap-nonce | wrapped-DEK + tag | content-nonce | ciphertext + tag`, stored as `<name>.enc`; plaintext deleted after successful encryption (best-effort).
- Decryption prompts for the passphrase; a wrong passphrase fails the GCM tag = clean error, nothing corrupted. Decryption restores the original name and keeps the `.enc` copy (recovery never destroys the sealed file).
- Crates: `aes-gcm`, `argon2`, `zeroize`, `getrandom` (M3: `pdf-extract`, `zip` with `deflate` only, `quick-xml` for extraction).

## Critical Invariants (Do Not Violate)

1. **Local only** — no file content, path, or metadata leaves the machine; the model runs on-device; no cloud APIs.
2. **The AI never touches files directly** — all disk operations go through the Rust script-tool layer, which validates paths and scope.
3. **Consent gates irreversible actions** — encryption (and deletion, if ever added) requires an explicit user yes; show the plan first.
4. **Model output is untrusted input** — validate every proposed operation (paths stay within the chosen folder; no traversal outside scope) before executing.
5. **Confidential files are never silently modified** — detection flags them; encryption only after approval; every action is reported.

## Key Conventions

- One writer per file (see `docs/parallel-workflow.md` for parallel sessions).
- Decisions that are hard to reverse, surprising, and involve a real trade-off get an ADR in `docs/adr/`.
