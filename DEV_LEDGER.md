# Development Ledger
**Last Updated:** 2026-11-18 17:20 | **Last Commit:** `pending` desktop: add the overflow bar

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up. GUI work is allowed.
**Source of truth: `negpy_egui/AGENTS.md`**; `MIGRATION_PLAN.md` is advisory.
This session: work 10 commits, targeted tests per commit, then the workspace run.

## Next Steps (Ordered)
1. [ ] E2: port `overflow_bar`, `sub_tabs`, `sort_button`, `flat_rows`, `elided_label`, `marks`; then `charts`.
2. [ ] E5: build the rail cards on the widget layer; the score gate is 0.80.
3. [ ] Move one asset family into `negfe/assets/` when its Rust code reads it.

## Completed (Recent)
- `pending` desktop: add the overflow bar with a pinned tab and a spill list.
- `pending` desktop: add the panel tab with the warm chip, icon-only width and edited dot.
- `46ae11a6` desktop: add the rail tab header.
- `4344b0b2` desktop: align field rows to the slider columns.
- `b677b361` desktop: add the split action.
- `e0499fa5` desktop: add the fold and its rail.
- `6776562c` desktop: add the step combo and card header.
- `1dad6412` desktop: add the slider and segmented choice.
- `ee6f4697` desktop: draw the shell glyphs from the icon map.
- `084a7484` desktop: add the phosphor icon map.
- `d1811edf` desktop: capture the contact sheet dialog.
- `da905e74` desktop: load the fixture frame into the capture.
- `6b5da499` desktop: build controls on the shared theme.
- `68998a7e` desktop: add the shared layout metrics.
- `0b3c2a4b` desktop: add the color vision palettes.
- `f5276a7f` desktop: add the UI font stacks.
- `fc49813d` desktop: add the theme token table.
- `2f007228` tests: add the GUI capture scripts.
- `4256b860` tests: add the binding golden GUI set.
- `dc7d2e6d` desktop: capture the golden matrix.
- `60762a7f` desktop: read the golden capture manifest.

## Verification
- `cargo fmt --all -- --check`, `cargo check --workspace` and
  `cargo test --workspace` pass after commit 10.
- The capture writes 20 PNGs; every layout score fails the 0.80 gate until the
  rail cards land. Mean 0.487, dialog 0.673, best tab 0.530.
- `cargo tree -p negpy-backend` shows no PyO3, numpy, egui or eframe.
- `git diff --stat -- negpy negpy_core` is empty.

## Audit
- **D10:** GPU engine, WGSL shaders, ImageProcessor cache coordination and the
  diptych join remain. `cam_xyz`/`camera_wb` are not threaded through
  `PipelineContext`; `linear_raw` parity is open.
- **D12:** real ONNX model verification and scanner hardware transports remain.
- JPEG XL and lossy WebP need encoders.

## Constraints / Gotchas
- Never edit `negpy/`; `negpy_core` stays its own workspace.
- Binding GUI reference: `negpy_egui/tests/gui/golden/tall/`.
- GUI parity is a layout score of 0.80, not pixel identity.
- The capture binary is `cargo run -p negpy-desktop --bin negfe-capture -- --golden tests/gui/golden/tall`.
- Generated captures live under `tests/gui/actual/` and stay untracked.
- The capture binary reads `NEGPY_GUI_FOLDER` and `NEGPY_GUI_PROFILE` first; the
  manifest `source` block is the fallback.
- The 10-commit request supersedes the 5-commit checkpoint.
- Headless desktop helpers live under `session/`, not as module filenames.
