---
description: Show parallel worktrees, reservations and pending teardowns
---
Current parallel workflow status:

!`pwsh -NoProfile -File .agents/skills/worktree-finish/scripts/Get-TaskStatus.ps1`

Summarize, in a few lines: active tasks, shipped tasks awaiting teardown, and anything that
looks stale (reservation with no worktree, or a worktree with no reservation). Take no other
action unless I ask — e.g. `-Mode cleanup` runs only when I say so.
