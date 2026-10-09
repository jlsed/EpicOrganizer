---
name: html-mockup-lab
description: Build several static, token-exact HTML mockups of an app screen (real fonts, real theme values, light/dark + key states) in an out-of-repo workspace, render them to 2x PNGs with headless Chrome, review the images, iterate on owner feedback, and present a combined option board for the owner to choose before any app code is touched. Use when the owner asks to "mockup/redesign a screen", wants design options for a layout or a visual direction, says "show me the designs before building", or when a redesign is too large to code blind. The no-build fast lane for screen-level design exploration.
---

# HTML mockup lab

Produce quick, **token-exact static mockups** of a screen as real HTML, render them to images with
headless Chrome, and let the owner choose — before any app code changes. This is the fast lane of
UI redesign: no app builds, no device, seconds per iteration; the price is that gestures, real
input behavior, real data and platform chrome cannot be judged from the images.

## When to use / when not

- **Use** for screen-level redesigns and layout exploration (the editor, Home, a settings flow),
  for "show me a few options" rounds, and for drafting a whole-app direction before committing.
- The winning direction then lands through the real build — adapt the hand-off once the stack is
  locked (there is no `emulator-visual-check` companion installed in this project).
- **Do not use** as final validation — static images never prove behavior, input anchoring, gesture
  feel or performance. Say so in the report.
- **Do not use** for a single token tweak (edit the theme token source and stop) or for feature work.

## Phase 0 — Scope and references (no files written)

1. Read the memory bank core files and any project design docs (`docs/design-system.md` and the
   like, when they exist) — the mock must obey the binding standard (or explicitly propose
   exceeding it).
2. **Ask the owner the dials** with the question tool before drawing anything: which surface(s), what
   the mockups are for (static images vs runnable variants), whether references come from the owner
   or proposed by you, and how much layout freedom the fields have (all fields stay reachable by
   default). Optional: run `grilling` when the scope is large.
3. **Analyze the references**: write a short `pattern → app component` mapping table (what to borrow
   and where it lands). References drive layout and treatment — **not** the palette, unless the
   owner says otherwise.
4. **Ground in the real code**: read the app's theme/token sources and the target screen (stack TBD —
   substitute the real paths once it is locked), so the mock recreates the app, not a generic screen.

## Phase 1 — Workspace (outside the repo)

```
C:\Users\sed\AppData\Local\Temp\opencode\<name>-mockups\
  mock.css                # shared, per-theme CSS variables
  variant-a.html          # one file per direction; N phone frames behind ?i=
  variant-b.html
  board.html              # combined option board (embeds the rendered PNGs)
  fonts\*.ttf             # copy the app's real font faces
  shots\*.png             # renders; never committed
```

Copy the app's real font faces from the stack's font assets once known. **Never write inside the
repo during the mockup phase** — raw boards stay out of git.

## Phase 2 — Build the frames

- **Device-sized frames**: one `.phone` div per state at 360×800 CSS px (desktop surfaces: use the
  real target viewport).
- **Frame selector, not scrolling**: each variant file holds N frames; show only the requested one —
  `.phone { display: none }` / `.phone.show { display: flex }` + `?i=` in the URL. Do **not** stack
  frames and scroll: under headless `--virtual-time-budget` the tiles past the first viewport come
  out blank (known gotcha, `references/render-and-verify.md`).
- **Exact tokens**: define the app's values as CSS variables per theme class (`.theme-x` /
  `.theme-x.dark`) straight from the app's theme sources, and use the app's shape/spacing/type
  values read from those files. A mock value that is not the app's value is a lie.
- **States that decide the design**: light + dark, and the variants that change layout (e.g. filled
  vs empty, transfer vs single-item, focused/keyboard state, 200 % font check where it matters).
- **Real content**: real labels and real-looking amounts and names — no lorem ipsum.
- Follow the current design standard while mocking; when you deliberately go outside it (a new
  direction), mark it as such on the board and in the final report.

## Phase 3 — Render

Serve the folder over **http** (fonts do not load reliably over `file://`) and screenshot each frame
with headless Chrome at 2× DPR. Exact commands, the board pass and every gotcha:
`references/render-and-verify.md`.

## Phase 4 — Review and iterate

Read back **every** PNG with the Read tool before judging anything (it renders images). Fix, re-render,
re-read — never claim "it should look right". Iterate on owner comments by editing the HTML only;
each loop is seconds, not builds.

## Phase 5 — Present the option board

One combined board (all directions × states) + the individual PNG paths, a one-line description per
direction, and a short "your call" list (which one, and the specific open decisions). The owner
decides the look; rejected options are reported when the direction is archived.

## Phase 6 — Promote the winner

The chosen mock becomes the spec: land it in the app through the normal slice cadence (tokens →
components → screen), validate in the real build once the stack is locked, and update the design
standard/ADR if the direction changed a rule. Versioning is currently off — no version bump. The
mock workspace is scratch — delete it once the direction is landed or explicitly archived.

## Report format

Each phase ends with a short report: **files changed** (or "none"), **commands run**, **screenshots
with paths + the variant/theme each shows**, **evidence for visual claims** (the image path), and
**the next decision for the owner** as a question with options.

## Hard rules

1. No repo writes during mocking; raw images stay in the temp workspace.
2. Mock token values mirror the app's theme sources exactly unless the mock is explicitly exploring
   a new standard (then label it).
3. Every visual claim is backed by a viewed image.
4. All existing fields/states stay reachable unless the owner relaxed that.
5. Static mockups do not validate gestures/input/performance — the winner still gets the real pass.
6. The owner chooses the look; bring facts, candidates and evidence.

## Related skills

`grilling` (Phase 0 for big scopes), `impeccable` (visual-system critique and direction),
`code-review` + `tdd` (landing the winner), `frontend-design` (anti-templated visual direction).
