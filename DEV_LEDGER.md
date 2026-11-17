# Development Ledger
**Last Updated:** 2026-11-18 10:35 | **Last Commit:** `4256b860` tests: add the binding golden GUI set

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up. GUI work is **allowed**.
NegFe is the target name; the folder rename is opportunistic.
**Source of truth: `negpy_egui/AGENTS.md`**; `MIGRATION_PLAN.md` is advisory.

## Next Steps (Ordered)
1. [ ] E0: commit the Python capture and comparison scripts.
2. [ ] E1: add the theme token module and its contrast tests.
3. [ ] E1: port `view.styles.*` theme, fonts, color_vision and templates.
4. [ ] Move one asset family into `negfe/assets/` when its Rust code reads it.

## Completed (Recent)
- `4256b860` tests: add the binding golden GUI set.
- `dc7d2e6d` desktop: capture the golden matrix.
- `60762a7f` desktop: read the golden capture manifest.
- `6cce2266` desktop: capture screenshots as PNG.
- `7605867d` assets: scan crosstalk profile files.
- `303cb18c` metadata: read a source EXIF block.
- `4b0803d3` export: attach an Exif IFD to TIFF.
- `33d95ab4` export: embed a metadata plan.
- `29765ca1` export: render a file at full resolution.
- `1a091aca` loaders: read an embedded RAW preview.
- `63991661` render: decode a preview on camera WB.
- `6e4276b5` export: encode a lossless WebP.
- `85706b4f` scanning: register the scanner backends.
- `e5fc7aad` capture: decode sensor RGB for metering.

## Verification
- Targeted tests passed per commit; `cargo fmt --all -- --check` clean.
- First layout score: 0.429 on `02_roll_top.png` (gate 0.80).

## Audit
- **D10:** GPU engine, WGSL shaders, ImageProcessor cache coordination and the
  diptych join remain. `cam_xyz`/`camera_wb` are not threaded through
  `PipelineContext`; `linear_raw` parity is open.
- **D12:** real ONNX model verification and scanner hardware transports remain.
- JPEG XL and lossy WebP need encoders.

## Constraints / Gotchas
- Never edit `negpy/`; `negpy_core` stays its own workspace.
- Binding GUI reference: `negpy_egui/tests/gui/golden/tall/`, 19 window captures
  at 1920x1600 plus the Contact Sheet dialog at 1080x760.
- GUI parity is a layout score of 0.80, not pixel identity. Extract layout from
  `theme.py`, `templates.py`, `main_window.py`, `right_panel.py`, `controls_panel.py`.
- E0 comes first: the Rust capture must write every golden PNG and a manifest.
- The capture harness copies the profile and suppresses the restore prompt.
- NegFe owns `negfe/assets/` and `~/Dokumenty/NegFe/`; copy one family at a time.
- Fingerprints and export bytes are pinned to Python.
- The 10-commit request supersedes the 5-commit checkpoint.
- Headless desktop helpers live under `session/`, not as module filenames.
