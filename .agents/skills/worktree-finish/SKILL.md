---
name: worktree-finish
description: Reconcile, merge and tear down a finished EpicOrganizer task worktree in the parallel workflow. Use when a task's build is done and validated — rebases on main, applies the inbox record to the memory bank, creates the single commit on main (version-tagged only if a version block is configured), and prints the teardown step. Also use with `-Mode teardown` or `-Mode abandon` to remove a worktree (finished, or abandoned mid-task). One finish at a time; run finish from the task session, teardown/abandon from outside the worktree.
---

# Worktree finish

Closes a task from `docs/parallel-workflow.md` §3 (step 5) and reproduces the repo's
one-commit-per-slice history. Run **finish** from the task worktree, then
**teardown** from the main repo (Windows locks a worktree while its session holds it open).

## 1. Before running finish (in the task session)

The branch never wrote to the memory bank; its inbox holds the record. Do this first:

1. Copy the inbox record into the worktree's inherited memory-bank files — this is the
   reconciliation step:
   - `memory-bank/progress.md` — add the new **Current Status** entry at the top (dense
     slice paragraph: date, what/why, validation summary, evidence).
     Update the milestone table / What Works / What's Left / Known Issues only if the slice
     changed them.
   - `memory-bank/activeContext.md` — add the new **Current Phase** entry at the top; update
     the **Current State Snapshot** lines the slice changed (version, test counts, DB/CSV
     contract, dependencies); append any **Learnings**.
   - `memory-bank/archive/<milestone>-history.md` — append the slice's validation record.
   - `memory-bank/techContext.md` / `systemPatterns.md` / `productContext.md` — only the
     deltas the inbox lists (DB version, dependencies, contracts, capability index).
   - **Rule 3**: every checklist item you check off must carry verification evidence
     (artifact names, `file:line`, command output). Leave unverifiable items unchecked and
     report them.
2. Confirm the validation evidence is in the inbox: the project's configured test commands
   (`commands.test` / `commands.release` in `worktree.config.json` — currently unset, so
   record what was actually run) and any build output. Device checks are optional,
   owner-invoked, never blocking.
3. Commit the reconciliation (`git add -A; git commit -m "wip: reconcile memory bank"`) so
   the worktree is clean. Any other uncommitted work must be committed or discarded.
4. Delete the inbox file (`memory-bank/inbox/<task-id>.md`) — its content now lives in the
   memory bank; it is gitignored anyway.

## 2. Run finish

```powershell
pwsh .\.agents\skills\worktree-finish\scripts\Complete-TaskWorktree.ps1 `
  -Type fix -Summary "keep session state on reload" `
  -Body "Root cause + what changed, and why this approach.

Task <task-id> (ADR 0001). Validation: <commands run + results>."
```

What the script does: finish lock → clean-tree checks → `git rebase <base>` (a conflict
isolated to the configured version file, caused by a sibling finishing first, is
auto-resolved; any other conflict aborts and expects manual resolution + rerun) → final
version (reserved; renumbered above main if a sibling shipped a higher code; skipped
entirely when `worktree.config.json` has no `version` block) → configured tests
(`commands.test`, plus `commands.release` with `-ReleaseBuild`) → squash all WIP commits
into one `[vX.Y.Z (code): ]type: summary` commit → fast-forward the base branch (linear
history) → ledger update. If the base branch moved during the run it rebases the single
commit and retries.

Useful switches: `-VersionKind step` to promote the task to the next step at finish;
`-NoVersionBump` for docs-only tasks; `-SkipTests` only for a pre-validated emergency;
`-DryRun` to preview.

## 3. Teardown

From the main repo (or any shell outside the worktree):

```powershell
pwsh .\.agents\skills\worktree-finish\scripts\Complete-TaskWorktree.ps1 -Mode teardown -Slug <slug>
```

Removes the worktree (`--force`; runs the configured `commands.stop` and retries if Windows
holds locks), deletes `task/<slug>`, deletes the task's launcher in
`E:\Hackathon\EpicOrganizer-wt\.launchers\`, and clears the ledger reservation.

Close every finished task at once (after closing their windows):

```powershell
pwsh .\.agents\skills\worktree-finish\scripts\Complete-TaskWorktree.ps1 -Mode cleanup
```

`cleanup` removes every worktree whose ledger state is `shipped` (worktree, branch, launcher,
reservation); locked ones are skipped and reported so you can close them and rerun. See what
is pending first:

```powershell
pwsh .\.agents\skills\worktree-finish\scripts\Get-TaskStatus.ps1
```

(or the `/task-status` command in opencode).

## 4. Abandon a task

```powershell
pwsh .\.agents\skills\worktree-finish\scripts\Complete-TaskWorktree.ps1 -Mode abandon -Slug <slug> [-KeepBranch]
```

Removes the worktree without merging and releases the reservation (ADR numbers may gap —
acceptable). Same "run from outside the worktree" rule.

## Notes

- OpenCode shortcut: in the task session, `/end-task` walks this whole flow (validate →
  reconcile → finish → report the teardown command).
- Project knobs live in `worktree.config.json` (tracked): version file + regexes,
  `commands.test` / `commands.release` / `commands.stop`, `secretFiles`, `worktreeRoot`.
  Edit it on main, never from a task branch; `worktree-overlap-check` treats it as
  main-managed.
- Never bypass the script's gate by merging `task/<slug>` into main yourself: the memory-bank
  reconciliation and ledger update must land in the same single commit.
- If a version file is configured, its conflict at rebase is expected and safe; do not
  hand-resolve it with `git checkout --theirs` on the whole file unless you first verify the
  branch had no other intended changes in the version file.
