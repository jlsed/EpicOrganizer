---
description: End the current task worktree — reconcile the memory bank, run worktree-finish, merge into main
---
Follow the **worktree-finish** skill to close this task. Optional overrides from me: $ARGUMENTS

Checklist:

1. **Validate** — the work is done and the inbox (`memory-bank/inbox/<task-id>.md`) holds the
   validation evidence for the project's configured commands (`commands.test` from
   `worktree.config.json`, plus `commands.release` when relevant; both are currently empty —
   record whatever was actually run). Run anything missing now and record the results; leave
   unverifiable checklist items unchecked and report them.

2. **Reconcile** — copy the inbox record into this worktree's memory-bank files (progress.md
   top entry; activeContext.md entry + snapshot deltas; the milestone archive record;
   techContext / systemPatterns / productContext deltas; checklist changes with verification
   evidence). Delete the inbox file, then commit so the tree is clean:
   `git add -A; git commit -m "wip: reconcile memory bank"`.

3. **Finish** — compose a conventional type, summary and body from the inbox and run:
   `pwsh ./.agents/skills/worktree-finish/scripts/Complete-TaskWorktree.ps1 -Type <feat|fix|refactor|test|docs|chore> -Summary "<summary>" -Body "<body>"`
   Add `-ReleaseBuild` when a release gate is configured and should rerun. On any real
   conflict, resolve it in the worktree (`git add` + `git rebase --continue`) and rerun finish.

4. **Report** the merged commit and ledger state, plus any build artifact path produced inside
   the worktree — copy it somewhere safe before teardown, which deletes the worktree. Then
   tell me to close this window and run teardown from the main repo:
   `pwsh ./.agents/skills/worktree-finish/scripts/Complete-TaskWorktree.ps1 -Mode teardown -Slug <slug>`
   (or `/task-status` in main, then `-Mode cleanup`). Never remove this worktree from inside
   it — Windows locks the directory while this session is open.
