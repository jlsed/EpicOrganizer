---
name: worktree-overlap-check
description: Pre-flight collision gate for the EpicOrganizer parallel workflow. Use right after Plan Mode finalizes the target files, and again whenever the plan or file set changes mid-build — whenever more than one task worktree exists, before Build Mode starts or resumes. Compares this worktree's planned files against every other active worktree's declared scope, dirty files, and committed diff, ignoring main-only files managed by spawn/finish. GREEN means safe to build; RED names the overlapping files and owners and means wait or re-scope. Do not skip it while parallel worktrees are active.
---

# Worktree overlap check

The pre-flight gate from `docs/parallel-workflow.md` §3. Run it in the task session, after
the plan is final and before Build Mode (and again if the file set changes).

## Run

```powershell
pwsh .\..\.agents\skills\worktree-overlap-check\scripts\Test-Overlap.ps1
# or, if the skill is reachable from the repo root:
pwsh .\.agents\skills\worktree-overlap-check\scripts\Test-Overlap.ps1
```

Optional: `-PlannedFiles a/b.kt,c/d.kt` to add files not yet written into the inbox.

## What it compares

- **This worktree's planned files** — the `## Planned files` section of
  `memory-bank/inbox/<task-id>.md` (one repo-relative path per line; globs allowed), plus
  `-PlannedFiles`.
- **Every other active worktree**: its declared files (its inbox), its dirty files
  (`git status --porcelain`), and its committed diff vs `main`
  (`git diff --name-only main...<branch>`).
- **Excluded from hard conflicts** (main-managed, see `docs/parallel-workflow.md` §2):
  `memory-bank/**`, the version file configured in `worktree.config.json` (and the config
  itself), `.parallel/**`, the configured secret files (`.env`, `.env.local`), `.agents/**`,
  `.clinerules/**`, `docs/adr/**`.
- **Advisory** (not blocking): same-directory different-file overlaps; `main` moved ahead
  of this branch (finish will rebase).

## Decision gate

- **GREEN (exit 0)** — no overlapping files. Proceed to Build Mode.
- **RED (exit 1)** — the listed files overlap. Stop: wait for the other task to finish or
  re-scope the plan so the file sets are disjoint. Rerun after re-scoping.

The result (result, planned list, conflicts, main-ahead) is appended to the task inbox so
the finish record shows the gate ran. Keep the inbox as the plan record — if you change
the plan, update the `Planned files` section, then rerun.
