# Development Ledger
**Last Updated:** 2026-11-13 10:00 | **Last Commit:** `c1cf2ada backend: add runtime lens metadata model`

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up.
**Source of truth: `negpy_egui/AGENTS.md`**; `MIGRATION_PLAN.md` is advisory.
User override: 10 commits this session; targeted tests per commit, workspace test after commit 10.
**Audit rule:** a merged Rust file is not proof of parity; symbol-level gaps below remain.

## Audit (After Commit 10)
- D2: corrected: `features/lens/models.py` is runtime lens metadata, not a WorkspaceConfig section. Ported now. D1, D3-D5 unchanged.
- D6: missing `read_exif_from_file`, bounded DNG/TIFF/embedded previews, `NonStandardFileWrapper`, supported demosaic list; lens metadata readers/opcodes/bind.
- D7: missing exposure `auto_sliders`, `placement`, analysis strip/ring/vectorscope helpers; process `fade`, `fade_auto_red`, `fade_estimate`, `logic` tokens, sensor `film_base`, chart Lab/LM fit and capture-color source-profile helpers.
- D8: missing geometry `skew`, `batch_autocrop`, film-border/edges, `film_and_picture`, `rebate_mask`, keystone inverse/solve; finish `rebate_tone`.
- D9: absent flatfield, HDR, lens logic/warps, RGB scan, stitch, transparency, cyanotype, lith, sabattier, pick-better.
- D10: absent CPU engine wiring, GPU engine, display/color management, WGSL shaders, parity tests.
- D11: absent export, scanning, film-edge and templating.
- D12: remaining migrations positive_auto, roll_fields, flatfield, flatfield_layout, gear_presets; absent thumbnails, embeddings, semantic/search, half-frame, gear assets.
- D13/E1-E8: absent desktop state glue and the egui UI.

## Completed (This Session)
- D12: Positive-frame auto-meter migration (tests green).
- `e6294ab3` D12: bounds, Cast Removal and source-color sweeps.
- `d9c0ce50` D12: hash rehome and ambiguity.
- `cf05350b` D12: normalization-roll baselines.
- `9db52d4a` D7: process path and decode tokens.
- `19c052be` D7: exposure analysis, stats and densitometer.
- `feff2dad` D7: process hue, sensor, capture color and chart patches.
- `7b5482df` D8: Lab resize sharpen and retouch split/tokens.
- `05c7b2c0` D8: crop keys, rect transforms and flip/drag.
- `effa9cf2` D6: IR planes and demosaic/camera helpers.
- D2: runtime lens metadata model (HEAD).

## Next Steps (Ordered)
1. [ ] D12: finish the five missing asset migrations.
2. [ ] D7: close exposure auto/placement, fade, process logic and chart gaps.
3. [ ] D8: port skew, batch auto-crop and film-border helpers.
4. [ ] D6: port bounded preview and lens-reader units.
5. [ ] D9, D10, D11, D12 assets, then D13/E1-E8.

## Constraints / Gotchas
- Full workspace test: 2026-11-13 09:00, passed with `cargo test --workspace`.
- Never edit `negpy/`; `negpy_core` stays its own workspace.
- Subject <=50 chars, body wrapped at 72, no trailing period; split on "and".
- Anti-loop: 3 failed fixes => STOP and ask for human guidance.
- D2 tokens use canonical JSON, not Python-interchangeable.
- D6: `fff_loader` returns None for SGI LogLuv until RLE/tables land.
- D6: JXL fixtures use `imagecodecs`; 16-bit stays `source_u16`.
- Retouch tokens use canonical JSON and SHA-256, not Python repr and SHA-1.
- Clippy `-D warnings` still fails on existing storage/rolls code.
