---
name: worktree-spawn
description: Spawn an isolated git worktree and memory-bank inbox for one task so several AI sessions can build in parallel without colliding. Use at the start of every task in the pipelined worktree workflow — when the owner names a task to start, says "spawn a worktree", "start a parallel session", "new task worktree", hands over a task card, or wants to work on main without generating collisions. Also use for `-ReserveAdr` when a running task discovers it needs an ADR number. Not for edits to an existing task session.
---

# Worktree spawn

Creates the isolated sandbox for one task in the pipelined worktree workflow. Full design:
`docs/parallel-workflow.md`. Run it from the main repo (or any directory inside the repo),
either in a short main session or directly in a terminal. The script resolves the repo from
the current working directory, so an in-repo copy and a master copy under
`~/.agents/skills` both work.

## What it produces

- A worktree at `<worktreeRoot>\<slug>` (default: sibling `<repo>-wt`) on branch
  `task/<slug>` from the base branch.
- Copies of the untracked secret files listed under `secretFiles` in
  `worktree.config.json` (e.g. `.env`, `.env.local`).
- **If the config has a `version` block:** the next reserved version applied to the
  configured version file and committed (`chore: reserve vX.Y.Z for <task-id>`), so branch
  builds stay installable and never regress. Without a `version` block, versioning is off
  and the branch starts clean.
- An optional reserved ADR number for `docs/adr/<NNNN>-<slug>.md`.
- `memory-bank/inbox/<task-id>.md` — the task's only memory-bank write (gitignored).
- A ledger entry in `<repo>\.parallel\ledger.json` (gitignored).
- A launcher script in `<worktreeRoot>\.launchers\<slug>.ps1` and, by default, a new Windows
  Terminal window in the worktree running opencode with the kickoff prompt queued.

## Intake

Ask the owner (or take from the request) — keep it short:

1. **Slug** (`^[a-z0-9][a-z0-9-]*$`) and **Title**.
2. **Goal / acceptance criteria** — one or two sentences.
3. **Scope hints** — the subsystem/files likely involved (used by overlap-check).
4. **Version kind** — `sub` for fixes/small changes (default), `step` for a feature-sized
   step, `milestone` only when starting a new milestone line. Ignored when versioning is off.
5. **ADR needed?** — if the change is design-decision shaped.

## Run

```powershell
pwsh .\.agents\skills\worktree-spawn\scripts\New-TaskWorktree.ps1 `
  -Slug fix-session-state -Title "Fix session state" `
  -Goal "State survives a reload." `
  -Scope "src/session/**" `
  -NeedsAdr
```

Rules the script enforces: repo clean and on the configured base branch; worktree path and
branch must not exist. It never fetches (local-only). Use `-DryRun` to preview,
`-NoVersionBump` for documentation-only tasks (no-op while versioning is off), `-VersionKind`
only when a `version` block is configured, `-WorktreeRoot` to override the root.

By default `-Open opencode` opens a new Windows Terminal window in the worktree running
`opencode --prompt '<kickoff>'` ("read the memory bank + inbox, plan, run
worktree-overlap-check before Build Mode"), so the task session starts briefed. `-Open shell`
opens a plain terminal; `-Open none` skips the launch (use when a script or agent must not
spawn windows). If `wt.exe` is missing the script falls back to a plain PowerShell window; if
the launch fails it prints the launcher path to run manually.

Mid-task ADR discovery:

```powershell
pwsh .\.agents\skills\worktree-spawn\scripts\New-TaskWorktree.ps1 -ReserveAdr -Slug <slug>
```

## Project config

`worktree.config.json` at the repo root (tracked, so every clone gets it):

```json
{
  "mainBranch": "main",
  "worktreeRoot": "",
  "secretFiles": [".env", ".env.local"],
  "commands": {}
}
```

- `secretFiles` — untracked files copied into every new worktree.
- `version` — optional block; add it to enable versioned commits (scheme `milestone`
  `0.M.S[.sub]` or `semver` `X.Y.Z`; regexes read/write the version file). Omit the block to
  disable versioning — the current state.
- `commands` — used by `worktree-finish` as the test/release gate and the lock-release hook
  (fill `test`/`release`/`stop` once the stack is locked).
- `worktreeRoot` — where task worktrees are created; empty = sibling `<repo>-wt`.

## After spawning

The auto-launched window (or the printed kickoff for `-Open none`) starts the task session:
read all memory-bank core files plus the inbox, plan, write the **Planned files** list and
plan notes into the inbox, and run `worktree-overlap-check` before Build Mode. Branches must
not edit the main-only files listed in `docs/parallel-workflow.md` §2 (`memory-bank/**`
outside the inbox, the configured version file, the root rule files).

Owner shortcut (opencode): run `/new-task` in a main-repo session — it does the intake, runs
this script, and the auto-launched session comes up briefed. `/task-status` lists worktrees
and states.

If the machine will run several builds at once, remember each build wants its own memory;
cap workers if memory is tight.
