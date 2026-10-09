# Parallel worktree workflow

> Status: adopted for EpicOrganizer (copied from KaneTracker, 2026-10-09). Skills:
> `worktree-spawn`, `worktree-overlap-check`, `worktree-finish` (`.agents/skills/`). Binding
> rules: `AGENTS.md` Rule 4. Versioning is currently **off** (no `version` block in
> `worktree.config.json`) — ADR reservations work, version reservations do not.

The cadence: several independent tasks run at once, each in its own git worktree, and are
integrated one at a time through a deterministic finish step. The failure mode this design
eliminates: two sessions editing the same single-writer files. The rule is **one writer per
file**: branches write only their own code and their own inbox; `main`'s single-writer files
are reconciled once, at finish.

## 1. Artifacts

| Artifact | Where | Notes |
| --- | --- | --- |
| Project config | `<repo>\worktree.config.json` (tracked) | version file + regexes, test/release/stop commands, secret files, worktree root; absent `version` block = versioning off (current state) |
| Task worktree | `<repo>-wt\<slug>` (default; sibling of the main repo) | branch `task/<slug>`; created by `worktree-spawn` |
| Task inbox | `<worktree>\memory-bank\inbox\<task-id>.md` | **unique per task, gitignored**; the branch's only memory-bank write |
| Reservation ledger | `<repo>\.parallel\ledger.json` | gitignored; reserves each task's ADR number (+ version if configured) |
| Finish lock | `<repo>\.parallel\finish.lock` | serializes finishes; stale after 2 h |
| Launcher | `<repo>-wt\.launchers\<slug>.ps1` | opens the task's terminal/opencode session; deleted by teardown/abandon/cleanup |
| opencode commands | `.opencode/command/new-task.md`, `task-status.md`, `end-task.md` | owner shortcuts |
| Main repo | `E:\Hackathon\EpicOrganizer` | integration only — never a build session while tasks run |

`secretFiles` lists untracked files copied into every fresh worktree. The first build in a
worktree is cold (caches are per project dir).

## 2. Main-only files (branch must not edit them)

- `memory-bank/**` — all core files and `archive/**` are **read-only on branches**. The branch
  records everything it wants to say in its inbox; `worktree-finish` applies the inbox to
  `main`'s copies. (`memory-bank/inbox/` is the one exception — the branch owns its own inbox
  file.)
- ADR numbering — the number is reserved at spawn (`docs/adr/NNNN-<slug>.md` is created on the
  branch with that number).
- The version file configured in `worktree.config.json` (none yet) — if configured, the branch
  gets the reserved version applied by `worktree-spawn`; `worktree-finish` rewrites it to the
  final version.
- Root rule files (`AGENTS.md`, `.clinerules/**`) — change them only from a main session,
  never from a task branch.
- `worktree.config.json` itself — main-managed.

## 3. Lifecycle

```
1  worktree-spawn        (main repo)       → worktree + branch + inbox + reservations
2  Plan Mode             (task session)    → inbox "Planned files" + plan notes
3  worktree-overlap-check(task session)    → GREEN gate before Build Mode
4  Build Mode            (task session)    → code, tests, WIP commits
                                             → inbox holds the memory-bank record + evidence
5  worktree-finish       (task session)    → rebase, tests, ONE commit,
                                             ff-only merge of main, ledger update
6  worktree-finish -Mode teardown (main)   → remove worktree, delete branch, ledger cleanup
```

### Step 1 — spawn (`worktree-spawn`)

Input: a short task card (slug, title, goal/acceptance, scope hints, ADR needed or not). The
script:

1. Verifies the main repo is clean and on the configured base branch (local-only; no fetch).
2. Creates `<repo>-wt\<slug>` on `task/<slug>` from `main` and copies the untracked files
   listed in `secretFiles`.
3. If the config has a `version` block, reserves the next `versionName`/`versionCode` and
   applies it; **not configured here**. Optionally (`-NeedsAdr`) reserves the next ADR number
   in the ledger.
4. Writes `memory-bank/inbox/<task-id>.md` from the inbox template.
5. By default (`-Open opencode`) opens a new Windows Terminal window in the worktree running
   `opencode --prompt '<kickoff>'`; `-Open shell` opens a plain terminal, `-Open none` skips
   and just prints the kickoff.

### Step 2 — plan (task session)

Read the full memory bank (it is inherited from `main` as of spawn). Plan normally. Then fill
the inbox: goal, approach, risks, test plan, and the **Planned files** list (one repo-relative
path per line; globs allowed). `overlap-check` reads that section.

### Step 3 — overlap check (`worktree-overlap-check`)

Run after the plan is final and again whenever the planned file set changes. The script
compares this worktree's planned files against every other active worktree:

- its **declared** files (its inbox "Planned files"),
- its **dirty** files (`git status --porcelain`),
- its **committed** diff vs the base branch (`git diff --name-only main...<branch>`).

Main-only paths are excluded from hard conflicts (they are managed by spawn/finish):
`memory-bank/**`, `.parallel/**`, `.agents/**`, `.clinerules/**`, `docs/adr/**`, and the
configured version file (none yet). Same-directory adjacency is reported as an advisory
warning, and the script warns when `main` has moved ahead of the branch (rebase will be
needed). Result: `GREEN` (exit 0) or `RED` (exit 1) with the offending files and their owners;
the outcome is appended to the inbox.

**Red means stop**: wait for the other task, or re-scope the plan.

### Step 4 — build (task session)

Implement in the worktree, committing WIP freely (no memory-bank writes outside the inbox).
The required validation per session is whatever `commands.test` (and, when it matters,
`commands.release`) configure in `worktree.config.json` — **both are currently empty**, so
record manual evidence (`npm test`, build output, etc.) in the inbox until a stack lands and
the commands are filled in.

Before finishing, write into the inbox: validation commands + results, the memory-bank record
(progress/activeContext/archive/techContext deltas, checklist changes with Rule 3 evidence),
the ADR summary, and any learnings.

### Step 5 — finish (`worktree-finish`)

Run from the task session. The script:

1. Acquires the finish lock (one finish at a time) and checks clean trees.
2. `git rebase main`. Conflicts abort — resolve in the worktree, then rerun.
3. Squashes all WIP commits (`git reset --soft main`) and creates the single commit
   (`v{versionName} ({versionCode}): {type}: {summary}` when versioning is on, plain
   `{type}: {summary}` otherwise).
4. Runs `commands.test` as the gate and, with `-ReleaseBuild`, `commands.release` — skipped
   with a warning while the commands are empty.
5. Applies nothing else: the **agent** has already copied the inbox record into the
   worktree's memory-bank files (any time before step 2, or between runs; it is committed
   with the squash).
6. Fast-forwards `main` (`git merge --ff-only task/<slug>`) — linear history, no merge
   commit. If `main` moved, it rebases the single commit and retries.
7. Updates the ledger and prints the teardown command.

Then run, from the **main repo** (or any shell outside the worktree):

```
pwsh ./.agents/skills/worktree-finish/scripts/Complete-TaskWorktree.ps1 -Mode teardown -Slug <slug>
```

It removes the worktree (`--force` retry; `commands.stop` if a lock blocks removal), deletes
`task/<slug>`, deletes the task's launcher, and clears the ledger reservation. Final teardown
cannot always run from inside the worktree because Windows locks the directory while the task
session holds it open.

`-Mode cleanup` does the same for **every** shipped task in one run, skipping worktrees whose
sessions are still open (close them and rerun). `Get-TaskStatus.ps1` (or `/task-status`) lists
active vs shipped tasks before clean up.

### Abandoning a task

```
pwsh ./.agents/skills/worktree-finish/scripts/Complete-TaskWorktree.ps1 -Mode abandon -Slug <slug> [-KeepBranch]
```

Removes the worktree without merging, releases the reservation (ADR numbers may gap; that is
fine), and deletes the branch unless kept. Also run it from outside the worktree.

## 4. Reservations

- **ADR** (active): next unused number in `docs/adr/` plus outstanding reservations. The
  branch file is `docs/adr/<NNNN>-<slug>.md`. If a session discovers mid-plan that an ADR is
  needed, run `worktree-spawn -ReserveAdr -Slug <slug>` to add the reservation to the ledger
  and inbox.
- **Version** (inactive — no `version` block): when enabled, monotonic
  `versionCode` = `milestone * 10000 + step * 100 + sub` (`semver` and `milestone` schemes
  supported). Spawn reserves `max(shipped, other reservations) + 1`; finish re-checks against
  `main` and renumbers if a sibling already shipped a higher code.

## 5. Testing default

The project's configured commands are the required testing **per session**: `commands.test` in
`worktree.config.json`, plus `commands.release` when a release build matters. Both are empty
until a stack is chosen — until then, record the commands you actually ran and their results
in the inbox. Emulator/device validation is optional and owner-invoked; it is never required
to finish a task.

## 6. Gotchas

- **Build daemons** are shared per machine. Two worktrees can build in parallel, but cap jobs
  if the machine swaps. Removal may fail while a daemon holds file locks —
  `git worktree remove --force`, then run `commands.stop` (if configured) and retry.
- **One device/service**: if two worktrees expose the same app id/port, never run both at
  once mid-check; serialize shared-resource work.
- **The inbox is gitignored**: `git status` stays clean, but the inbox is not backed up by
  git — finish copies its content into the memory bank before the worktree is removed.

## 7. Portability (worktree.config.json)

The workflow is stack-agnostic through one tracked file at the repo root. Current contents:

```json
{
  "mainBranch": "main",
  "worktreeRoot": "",
  "secretFiles": [".env", ".env.local"],
  "commands": {}
}
```

- `version` — omit the block to disable versioning entirely (plain `type: summary` commits,
  no reservations). `scheme` is `milestone` (`0.M.S[.sub]`) or `semver` (`X.Y.Z`); the
  regexes read/write the version; `codeRegex`/`codeReplace` are optional.
- `commands` — executed by finish with the worktree as the working directory; `stop` is
  invoked only when a worktree removal is blocked by a lock. Fill in `test`/`release`/`stop`
  once the stack is locked.
- `secretFiles` — untracked files copied into each fresh worktree.
- The scripts fall back to sensible defaults (sibling `<repo>-wt` root, `main` branch) when
  the file is missing; a missing file never blocks the workflow.

## 8. Owner automation (opencode)

In a main-repo opencode session:

- `/new-task` — intake (slug/title/goal/scope/ADR) then run `worktree-spawn`; the script opens
  the task session window with the kickoff already queued.
- `/task-status` — runs `Get-TaskStatus.ps1` and summarizes active/shipped tasks.

In a task-session window:

- `/end-task` — reconcile the inbox into the memory bank, then run the `worktree-finish` flow
  (validate, squash, fast-forward main) and report the teardown command.

opencode loads project commands from `.opencode/command/` at startup — quit and restart
opencode after editing them. Other harnesses use the skills directly.
