# EpicOrganizer — Project Brief

## Mission

A desktop app that organizes a user's files by natural-language instruction, orchestrated end-to-end by a fully local AI. Privacy is the headline: the AI runs on the user's machine (Llama 3.1 8B), reads file contents, plans the organization, and executes it through script tools — with a confidential-file detection pass that can encrypt sensitive files on request.

## Core Requirements

- **Natural-language instructions**: the user tells the AI how to organize (e.g. "within this folder, move all images into a new folder"); the AI plans the operations.
- **AI orchestration via script tools**: the AI lists directories, reads file contents, creates folders, moves/renames files — through a bounded tool layer, not raw shell.
- **Confidentiality detection**: while organizing, the AI identifies likely-confidential files and surfaces them to the user.
- **Optional encryption**: flagged files can be encrypted (user confirms; the AI drives the encryption flow end-to-end).
- **Read file contents**: content reading is what powers smart categorization and confidentiality detection.
- **100% local**: no cloud calls; the privacy story is the differentiator.

## Shipped Milestones (one line each; per-slice detail in `memory-bank/archive/`)

- None yet — 1-day hackathon build.

## Current

- Decisions locked 2026-10-09 (hackathon day). No code yet. Stack in `techContext.md`; architecture in `systemPatterns.md`; open questions in `activeContext.md`.

## Success Criteria

- Demo: point the app at a messy folder, give one natural-language instruction, watch the AI plan + organize correctly.
- Demo: a confidential file is detected, the user approves, the app encrypts it — and the decryption path proves the file is recoverable.
- Everything runs locally — airplane mode proves it.

> Per-slice detail lives in `memory-bank/archive/`; decisions in `docs/adr/`.
