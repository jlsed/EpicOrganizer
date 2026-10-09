# EpicOrganizer — AI Session Rules

These rules apply to **every AI-assisted session** on this project, regardless of which AI harness (Cline, Claude Code, Codex, OpenCode, or any other) is in use. They are binding instructions, not suggestions. (Adapted from KaneTracker's session rules; stack-specific rules removed until a stack is locked.)

---

## Rule 1: Documentation / Planning → Memory Bank & Docs Only

When the task is **documentation and planning only** (no code changes requested by the user), the AI MUST:

1. **ONLY modify files in these locations:**
   - `memory-bank/` — project memory bank files (`projectbrief.md`, `productContext.md`, `activeContext.md`, `systemPatterns.md`, `techContext.md`, `progress.md`)
   - `docs/` — documentation and ADRs
   - `archive/` — historical records
   - Root-level `*.md` files (`CONTEXT.md`, `AGENTS.md`, etc.)
2. **NEVER modify:** application source code, build configuration, or any other code/config/resource files.
3. **MAY read the codebase freely** for better context — reading is always allowed and encouraged.
4. **MUST use available skills** (in `.agents/skills/`) and **web searching** to find the best implementation patterns and documentation approaches before writing.

---

## Rule 2: Skills & Research (All Tasks)

For **every task** (code or documentation), the AI should:

- Check available skills in `.agents/skills/` for relevant guidance before implementing.
- Use web searching to verify best practices for the chosen stack.
- Prefer mainstream, well-documented solutions aligned with the project's locked stack (see `memory-bank/techContext.md`).

---

## Rule 3: Checklist Verification (Memory Bank Progress)

When the task involves **checking off, updating, or reporting status** of any checklist in `memory-bank/progress.md` (or any memory-bank file with task checklists), the AI MUST:

1. **Verify every unchecked item against the actual codebase** before checking it off — do not rely on the user's claim, the memory bank's own "shipped" labels, or milestone summaries alone.
2. **Verify by concrete evidence**: search for the actual function/file/route names listed in the item (e.g. `rg` for handlers, components, migrations, test files). An item is only "done" if its named artifacts exist.
3. **Leave genuinely-undone items unchecked** and report them explicitly to the user, even if the user believes they are done. Do not check an item to match expectations.
4. **Report the full audit**: list what was checked (with evidence), what was left unchecked (with reason), and any discrepancies found.

---

## Rule 4: Parallel Worktree Sessions

Tasks may run in parallel, one AI session per git worktree on `task/<slug>` (`E:\Hackathon\EpicOrganizer-wt\<slug>`), integrated through the first-party skills `worktree-spawn` / `worktree-overlap-check` / `worktree-finish`. Full spec: `docs/parallel-workflow.md`.

1. **One writer per file.** A task branch writes only its own code and its own `memory-bank/inbox/<task-id>.md` (gitignored, unique per task). `memory-bank/**` (core + archive) and the root rule files are main-only — the finish step reconciles them. Never edit them from a branch.
2. **Reserved identity, finalize on main.** ADR numbers are reserved at spawn in `.parallel/ledger.json` (gitignored); `worktree-finish` finalizes them (renumbering if a sibling shipped first) and produces the single commit on `main` via fast-forward. Versioning is currently **off** (no `version` block in `worktree.config.json`) — commit messages are plain `type: summary` (`feat:`, `fix:`, `refactor:`, `test:`, `docs:`, `chore:`). Never merge a task branch by hand.
3. **Overlap gate.** Run `worktree-overlap-check` after Plan Mode and before Build Mode; `RED` means wait for the other task or re-scope.
4. **Serialized finish.** One finish at a time (`.parallel\finish.lock`); teardown/abandon run from outside the worktree. One session per worktree.

---

## Rule 5: Memory Bank Protocol

At the **start of every session**, read ALL memory-bank files — this is not optional. Update them when:

- a new project pattern is discovered,
- after implementing significant changes,
- when the user requests it (**update memory bank** — review ALL files).

Core files stay compact at status / invariants / next-step level; per-milestone detail lives in `memory-bank/archive/`.

---

## Project Quick Reference

- **Project**: EpicOrganizer — fresh hackathon project; stack not locked yet.
- **Root**: `E:\Hackathon\EpicOrganizer`
- **Stack**: TBD — record the decision in `memory-bank/techContext.md`; fill `commands` in `worktree.config.json` once chosen.
- **Memory bank**: `memory-bank/` (read ALL files at session start; per-slice history lives in `memory-bank/archive/`).
- **Skills**: `.agents/skills/` (installed via `npx skills`; see `skills-lock`-style provenance in each SKILL.md source notes).
- **Versioning**: disabled until a `version` block is added to `worktree.config.json` and a version file exists.
