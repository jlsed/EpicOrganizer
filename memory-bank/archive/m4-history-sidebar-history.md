# EpicOrganizer — M4 history-sidebar history

## 2026-10-09 — History sidebar + cross-session decrypt

- Task: `2026-10-09-history-sidebar` (branch `task/history-sidebar`). Goal: a fixed left rail of
  previously organized folders; clicking restores the folder + last instruction; sealed `.enc`
  files are listed and decryptable after an app restart (the M3 gap: the webview forgot them and
  the passphrase lived only in session memory).
- Decisions (grilled with the grill-me/grilling skill, 14 owner-approved): one entry per folder
  (canonical case-folded key; MRU, cap 50); Rust-managed JSON in the app-data dir; click = set
  root + prefill instruction; fixed rail; app-sealed files only (cumulative, deduped); record
  after `execute_plan` (any outcome); successful decrypt marks `restored` in place; missing
  folders/files flagged by cheap `exists` checks; **passphrase never stored** — cross-session
  decrypt prompts fresh (ADR 0002).
- Built: `src-tauri/src/history.rs` (store load/save with tmp+rename atomic replace under a
  process mutex; `apply_organize` / `apply_sealed` / `apply_restored` / `apply_remove` pure
  helpers; `HistoryEntryView` with `exists` flags; 8 unit tests), hooks in
  `src-tauri/src/commands.rs` (`execute_plan` gained `instruction`; `encrypt_files` records ok
  `.enc` outputs; `decrypt_file` gained `AppHandle` + marks restored), commands `list_history` /
  `remove_history` / `clear_history` in `lib.rs`. UI: `index.html` `.app-shell` grid + `<aside>`
  rail, `src/styles.css` rail styles (light/dark), new `src/history.ts` (render rows, expand
  sealed list, one-at-a-time inline passphrase prompt, per-row remove, clear-all via native
  `ask`), `src/organizer.ts` returns `selectRoot` and refreshes the rail after
  execute/encrypt/decrypt, `src/main.ts` wiring, `src/types.ts` `HistoryEntry` /
  `SealedFileEntry`.
- Files: `index.html`, `src/{styles.css,main,organizer,types,history}.ts`,
  `src-tauri/src/{history,commands,lib}.rs`,
  `docs/adr/0002-history-sidebar-cross-session-decrypt.md`.
- Validation: `npm run build` exit 0; `cargo check` zero warnings; `cargo test` 38 passed / 2
  ignored (was 30+2). Live smoke through WebView2 CDP (`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=
  --remote-debugging-port=9222`, PowerShell ClientWebSocket + `window.__TAURI_INTERNALS__.invoke`):
  encrypted `secret.txt` via IPC → `history.json` persisted → killed and relaunched the app →
  rail rendered the persisted entry → wrong passphrase showed a clean error → correct passphrase
  restored the exact 44 bytes, kept the `.enc`, flipped the badge to `restored` and the store to
  `restored: true` → rail click set the active root → per-row remove and `clear_history` worked;
  ports 1420/9222 free after teardown.
- Performance: n/a — one small JSON read-modify-write per action; decrypt cost unchanged from M3
  (Argon2id tens of ms + AES pass over content).
- Follow-ups: `commands.stop` still unset in `worktree.config.json`; stack-trio ADR still missing;
  the keychain-backed "remember passphrase" option is deliberately not implemented (ADR 0002
  alternatives).
