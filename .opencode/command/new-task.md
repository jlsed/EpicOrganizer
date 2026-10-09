---
description: Spawn a parallel task worktree and open a briefed opencode session for it (worktree-spawn)
---
Run the worktree-spawn skill for a new parallel task.

Arguments (may be partial): $ARGUMENTS

If slug, title, goal/acceptance, scope hints, or ADR-needed are missing, ask me with the question tool first — one round, then run. Slug rules: lowercase letters, digits and dashes.

Then run from the main repo:

`pwsh ./.agents/skills/worktree-spawn/scripts/New-TaskWorktree.ps1 -Slug <slug> -Title "<title>" -Goal "<goal>" -Scope "<scope>" [-NeedsAdr]`

Add `-VersionKind <kind>` only after a `version` block exists in `worktree.config.json` — versioning is currently off.

The script creates the worktree, the inbox and the ADR reservation, then opens a new Windows Terminal window in the worktree running opencode with the kickoff prompt queued. Report the worktree path, branch and ADR, then stop. Do not start building here.
