# ADR 0002 — Folder history and cross-session decrypt

- **Status:** Accepted
- **Date:** 2026-10-09
- **Task:** `2026-10-09-history-sidebar`
- **Deciders:** EpicOrganizer owner (grill-me session, 2026-10-09) + history-sidebar build session

## Context

After M3, sealed files were only reachable through the in-memory UI state of the session
that encrypted them. Closing the app made them effectively undecryptable for the user: no
surface listed the `.enc` files, and the passphrase was held in a JS session variable only.
The owner asked for a history sidebar of previously organized folders that includes sealed
files and makes them decryptable again after a restart.

Three design questions needed settling:

1. Where does history live, and what is one entry — a run or a folder?
2. Does the app remember the passphrase to make decryption prompt-free after a restart?
3. What, exactly, is recorded as a sealed file, and what state does a decrypt leave behind?

## Decision

1. **History is a Rust-managed JSON file** (`history.json` in the Tauri app-data dir),
   written atomically (temp file + replace) under a process-wide mutex. One entry per
   folder, keyed by the canonicalized, case-folded path so different spellings collapse;
   MRU order, capped at 50 entries. An entry stores the display path, last-organized time,
   last instruction, ok/failed operation counts, and sealed files. Recording is best-effort:
   a store failure never breaks the organize flow.
2. **The passphrase is never stored — not in the store, not in the OS keychain, not in the
   webview.** Cross-session decryption works because the `EORG1` envelope is self-contained
   (per-file salt + wrapped DEK); the user simply enters the passphrase again in the rail.
   The UI states this ("Passphrase (never stored)").
3. **Only files sealed through the app are recorded**, accumulating per folder and deduped by
   path; the folder list is not scanned for arbitrary `.enc` files. Each sealed file carries
   `sealed`/`restored` state; a successful decrypt marks `restored` and keeps both the sealed
   copy and the row (re-decryptable), consistent with ADR 0001's recovery policy.
4. **Dead paths are flagged, not hidden**: `list_history` includes cheap `exists` flags for
   the folder and each sealed file; missing folders render dimmed and unselectable, missing
   sealed copies show "sealed copy missing" and cannot be decrypted.

## Consequences

- Sealed files are decryptable after a restart as long as the user remembers the passphrase;
  losing the passphrase still means losing the data (unchanged from ADR 0001).
- No secret material is persisted anywhere; `history.json` holds only local paths and run
  metadata, keeping the local-only, no-cloud invariant.
- History cannot make decryption prompt-free; that trade-off is deliberate and can be
  revisited with a separate ADR if a keychain-backed option is ever wanted.

## Alternatives considered

- **Store the passphrase in Windows Credential Manager (`keyring` crate)**: rejected — adds a
  dependency and changes the threat model (a stored secret now guards all sealed files) for a
  convenience gain that the fresh-prompt flow already covers.
- **Webview `localStorage`**: rejected — not testable from Rust, cleared with webview data,
  and splits state management across layers.
- **`tauri-plugin-store`**: rejected — a new dependency for what is one small JSON file.

## References

- `docs/adr/0001-confidential-detection-and-encryption.md` — envelope + recovery policy
- `memory-bank/systemPatterns.md` — encryption flow
- `src-tauri/src/history.rs`, `src-tauri/src/commands.rs`, `src/history.ts` — implementation
