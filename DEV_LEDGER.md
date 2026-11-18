# Development Ledger
**Last Updated:** 2026-11-18 18:10 | **Last Commit:** `HEAD` desktop: fit the canvas and strip tiles

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up. GUI work is allowed.
**Source of truth: `negpy_egui/AGENTS.md`**; `MIGRATION_PLAN.md` is advisory.
This session completed the 10-commit budget. E5 remains open.

## Next Steps (Ordered)
1. [ ] E5: match the rail card stack to the golden set (headers, heights, folds);
   the 0.80 layout gate is still failing.
2. [ ] E5: fill the Metadata, Export and Scan cards, then the Frame card bodies.
3. [ ] E5: rerun every capture with the real profile and roll, not stub paths.
4. [ ] Move one asset family into `negfe/assets/` when its Rust code reads it.

## Completed (Recent)
- `8eccce30` desktop: fit the canvas and strip tiles to the golden boxes.
- `e2b8ad5c` desktop: fill the Analysis and Roll card bodies.
- `748665d9` desktop: align the canvas, strip and toolbar bands.
- `599fd35d` desktop: build the rail view on the shared controls.
- `5f07c2e6` desktop: add flat rows with fills, pills and ⋮.
- `fbfbdb23` desktop: add triage badges and elided labels.
- `5feb7a5a` desktop: add the sort control with its direction choice.
- `3779b8b6` desktop: add the sub-tab strip with its saved key.
- `c4eb0f2e` desktop: add the overflow bar with a pinned tab.
- `9445ab83` desktop: add the panel tab with chip and edited dot.

## Verification
- `cargo fmt --all -- --check`, `cargo check --workspace` and
  `cargo test --workspace` pass after commit 10.
- Capture writes 20 PNGs with the real frame. Mean 0.503, best 0.673; no image
  reaches the 0.80 layout gate.
- `cargo tree -p negpy-backend` shows no PyO3, numpy, egui or eframe.
- `git diff --stat -- negpy negpy_core` is empty.

## Constraints / Gotchas
- Never edit `negpy/`; `negpy_core` stays its own workspace.
- Binding GUI reference: `negpy_egui/tests/gui/golden/tall/`.
- GUI parity is a layout score of 0.80, not pixel identity.
- Capture with `NEGPY_GUI_PROFILE=/home/bruno/Dokumenty/NegPy` and
  `NEGPY_GUI_FOLDER=/home/bruno/WORK/svetovy_vylet/skeny/vv9`.
- Generated captures live under `tests/gui/actual/` and stay untracked.
- A collapsible body must advance the parent cursor by its measured height,
  or the next card overlaps it.
- A segmented choice hit-tests inside its own child UI; a parent-wide
  `allocate_rect(..., Sense::hover())` duplicates widget IDs.
- The 10-commit request supersedes the 5-commit checkpoint this session.
- Headless desktop helpers live under `session/`, not as module filenames.
