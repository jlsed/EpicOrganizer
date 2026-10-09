# EpicOrganizer — Product Context

## Why this exists

Files pile up and manual organizing is tedious. Existing AI organizers send file names and contents to the cloud — a non-starter for private documents. A local model can do the orchestration without anything leaving the machine.

## Target user

- Privacy-conscious desktop users with cluttered folders.
- Hackathon judges: the demo must show instruction → organized folder, plus confidential detection → encryption, all offline.

## Product principles

- **Local by default, always** — no network dependence; inference runs on-device.
- **User consent before consequential actions** — encryption (and anything irreversible) requires an explicit yes.
- **Show the plan, then execute** — the user sees the AI's intended file operations before they run; every action is reported.
- **Explainable flags** — a file marked confidential says why (matched name pattern/content), not just a score.

## Scope boundaries

- In scope: desktop app; folder-scoped organization; script tools (list/read/move/create/rename); confidential detection; encryption on approval.
- Out of scope (hackathon): cloud sync, multi-user, mobile, scheduled/background automation, full undo/versioning beyond basic reporting.
