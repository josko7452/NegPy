# Development Ledger
**Last Updated:** 2026-11-18 17:30 | **Last Commit:** `HEAD` desktop: build the Scan rail tab

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up. GUI work is allowed.
**Source of truth: `negpy_egui/AGENTS.md`**; `MIGRATION_PLAN.md` is advisory.
This session completed the 10-commit budget. E5 panel bodies and capture navigation are built.

## Next Steps (Ordered)
1. [ ] E5: correct the shared shell bands; the rail still scores below the
   0.80 layout gate.
2. [ ] E5: replace reference-only panel values with backend services as each
   panel gets wired to the controller.
3. [ ] E6: start the dialogs over the new rail panels.

## Completed (Recent)
- `986be01e` desktop: build the Export rail tab.
- `5d6a3e2d` desktop: build the Metadata rail tab.
- `00955bc8` desktop: build the Finish frame page.
- `a42e181a` desktop: build the Look frame page.
- `cb9bac09` desktop: build the Exposure frame page.
- `c03aac07` desktop: build the Geometry frame page.
- `6f4623f1` desktop: build the Library rail tab.
- `dcccd4d0` desktop: add the tab summary bar.

## Verification
- Targeted `cargo test -p negpy-desktop --lib view::rail` passes per commit.
- `cargo check --workspace` and `cargo test --workspace` pass after commit 10.
- The real-profile capture writes all 20 PNGs with the selected tab and
  sub-tab. Layout scores run 0.455 to 0.673; none reaches the 0.80 gate.
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
