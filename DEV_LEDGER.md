# Development Ledger
**Last Updated:** 2026-11-18 15:50 | **Last Commit:** `1dad6412` desktop: add the slider and segmented choice

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up. GUI work is allowed.
**Source of truth: `negpy_egui/AGENTS.md`**; `MIGRATION_PLAN.md` is advisory.
This session: work 10 commits, targeted tests per commit, then the workspace run.

## Next Steps (Ordered)
1. [ ] E0: open the Contact Sheet dialog from the capture and score it.
2. [ ] E2: add the slider group and the rail/group folds.
3. [ ] E2: port `view.widgets.*` over the theme factories.
4. [ ] Move one asset family into `negfe/assets/` when its Rust code reads it.

## Completed (Recent)
- E0 frame load: backend display frame, fixture source paths, fitted canvas texture.
- E0 dialog: 1080x760 Contact Sheet view and per-entry viewport resize.
- E1 icons: pin `egui-phosphor` 0.10 for egui 0.32 and add the named icon map.
- E1 glyphs: shell tabs and dialog controls read the icon roles.
- E2 start: compact slider and segmented choice over the shared theme.
- E2 forms: step combo and collapsible card over the shared header.
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
- Targeted tests passed per commit; `cargo fmt --all -- --check` clean.
- `cargo test --workspace` passed after the tenth commit.
- Capture with `NEGPY_GUI_FOLDER` and `NEGPY_GUI_PROFILE` shows the real frame.

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
