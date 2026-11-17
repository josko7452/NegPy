# Development Ledger
**Last Updated:** 2026-11-18 12:15 | **Last Commit:** `6b5da499` desktop: build controls on the shared theme

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up. GUI work is allowed.
NegFe owns `negfe/assets/` and `~/Dokumenty/NegFe/`; the folder rename is opportunistic.
**Source of truth: `negpy_egui/AGENTS.md`**; `MIGRATION_PLAN.md` is advisory.

## Next Steps (Ordered)
1. [ ] E0: load the reference frame and open the Contact Sheet dialog in the capture.
2. [ ] E1: add `egui-phosphor` icon mapping and replace text glyph placeholders.
3. [ ] E2: port `view.widgets.*` over the theme factories.
4. [ ] Move one asset family into `negfe/assets/` when its Rust code reads it.

## Completed (Recent)
- `6b5da499` desktop: build controls on the shared theme.
- `68998a7e` desktop: add the shared layout metrics.
- `0b3c2a4b` desktop: add the color vision palettes.
- `f5276a7f` desktop: add the UI font stacks.
- `fc49813d` desktop: add the theme token table.
- `2f007228` tests: add the GUI capture scripts.
- `4256b860` tests: add the binding golden GUI set.
- `dc7d2e6d` desktop: capture the golden matrix.
- `60762a7f` desktop: read the golden capture manifest.
- `6cce2266` desktop: capture screenshots as PNG.

## Verification
- Targeted tests passed per commit; `cargo fmt --all -- --check` clean.
- `cargo test --workspace` passed after the tenth commit.
- Layout scores 0.425-0.526 across the 20 captures (gate 0.80).
- E0 capture wrote 20 PNGs plus `manifest.json`.

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
- The capture shell paints the scored regions; frame loading and dialog capture remain.
- The 10-commit request supersedes the 5-commit checkpoint.
- Headless desktop helpers live under `session/`, not as module filenames.
