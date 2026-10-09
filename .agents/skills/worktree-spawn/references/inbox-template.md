# Task inbox — {{TASK_ID}}

- **Title:** {{TITLE}}
- **Branch:** {{BRANCH}}
- **Worktree:** {{WORKTREE}}
- **Reserved version:** {{VERSION}}
- **Reserved ADR:** {{ADR}}
- **Status:** plan-ready
- **Created:** {{CREATED}}

This file is gitignored and is the only memory-bank file a task branch may write.
`worktree-finish` never rewrites it: before finishing, copy the record below into the
worktree's `memory-bank/` files (they are inherited from `main` and untouched on the
branch), then commit and run finish.

## Goal / acceptance criteria

{{GOAL}}

## Scope hints

{{SCOPE}}

## Planned files

<!-- One repo-relative path per line (globs allowed). worktree-overlap-check reads this section. -->

## Plan notes

<!-- Filled during Plan Mode: approach, risks, test plan, ADR decision summary if needed. -->

## Progress log

<!-- Append during Build Mode: decisions, WIP commits, blockers, scope changes. -->

## Validation

<!-- Record the project's validation commands + results (see `commands` in worktree.config.json).
     Default expectation: the configured test command; the release/build command when relevant.
     Emulator/device checks are optional, owner-invoked, never blocking; serialize them when
     other worktrees are active. -->

Performance: <!-- n/a, or affected + evidence per the project's performance standard, if any -->

## Memory-bank record

### progress.md — Current Status entry

<!-- One dense slice paragraph, matching the existing entries. Include the version, date, scope,
     validation summary, and the Performance line. -->

### activeContext.md — Current Phase entry + snapshot deltas

<!-- Same dense style as existing entries; note any Current State Snapshot changes
     (version, test counts, DB version, dependency/CSV contract changes). -->

### archive record

<!-- Target file (memory-bank/archive/<milestone>-history.md) and the section text to append. -->

### other core-file deltas

<!-- techContext.md / systemPatterns.md / productContext.md updates, if any. -->

### checklist changes

<!-- Checklist/checkbox updates with Rule 3 verification evidence (file:line or command output). -->

## ADR

<!-- If an ADR number was reserved: decision summary, the docs/adr/<number>-<slug>.md file status,
     and any ADR references. Remove this section only if no ADR is needed. -->

## Learnings

<!-- Reusable gotchas for activeContext's "Learnings & Preferences". -->
