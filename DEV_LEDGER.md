# Development Ledger
**Last Updated:** 2026-11-13 06:30 | **Last Commit:** `pending backend: share the camera white-balance fold`

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up.
**Source of truth: `negpy_egui/AGENTS.md`**; `MIGRATION_PLAN.md` is advisory.
User override: 10 commits this session; targeted tests per commit, workspace test after commit 10.
**Audit rule:** a merged Rust file is not proof of parity; symbol-level gaps below remain.

## Audit (After Commit 10)
- D12: `roll_fields` lacks `migrate_crop_to_frames`, `with_frame_seed`, `_with_roll_crop`; missing `flatfield`, `flatfield_layout`, `gear_presets`.
- D12 assets: absent thumbnails, embeddings, semantic/search, half-frame, gear assets, capture/view/session glue.
- D7: `auto_sliders` done; missing `placement`, analysis strip/ring/vectorscope helpers; fade lacks density, estimate, auto-red and crossover; process logic lacks should-fold and chart Lab/LM fit.
- D8: missing geometry `skew` detection, `batch_autocrop`, film-border/edges, `film_and_picture`, `rebate_mask`; finish `rebate_tone`.
- D9: HDR done; missing flatfield, lens logic/warps, RGB scan, stitch, transparency, cyanotype, lith, sabattier, pick-better.
- D10: absent CPU/GPU engine wiring, display/color management, WGSL shaders, parity tests.
- D11: absent export, scanning, film-edge and templating.
- D13/E1-E8: absent desktop state glue and the egui UI.
- D6: missing `read_exif_from_file`, bounded previews, `NonStandardFileWrapper`, supported demosaic list; lens readers/opcodes/bind.

## Completed (This Session)
- `pending` D7: shares the camera white-balance fold.
- `979dd218` D7: applies and measures the fade crossover table.
- `b5ce7ae8` D7: builds the fade crossover offset table.
- `f8b811b2` D7: adds the fade crossover table helpers.
- `e1e0018f` D12: moves Flat Field profiles into sensor layout.
- `0020afa8` D12: migrates the legacy Flat Field profile table.
- `fb0b963d` D12: adds the Flat Field profile store.
- `76d31520` D9: ports flat-field gain, evenness and application.
- `69b0a136` D12: moves Crop onto frames.
- `c3ca9cf3` D12: migrates Positive-frame auto metering.
- `f7beab1b` D12: locks new roll-default fields.
- `ac3e738c` D12: splits baseline card locks.
- `c2898d0c` D12: moves Cast Removal onto frames.
- `7bcb8715` D12: finishes roll card split migrations.
- `1d76ce16` D7: ports fade and decode helpers.
- `b20bee5a` D8: ports keystone inverse and solver.
- `aaf70b44` D7: ports Auto slider read-outs.
- `26839c03` D9: ports HDR exposure choices.
- `HEAD` D9: ports HDR merge path.

## Next Steps (Ordered)
1. [ ] D7: use the measured gray midtone as the crossover zero.
2. [ ] D7: add fade range and auto-red helpers after transparency.
3. [ ] D12: port the gear-preset migration.
4. [ ] D7: port exposure `placement` and the remaining fade/estimate helpers.
5. [ ] D8: port skew detection, batch Auto Crop and film-border helpers.
6. [ ] D9: port flatfield, lens, RGB scan and stitch.

## Constraints / Gotchas
- Full workspace test: 2026-11-13 02:03, passed with `cargo test --workspace` after a 51m43s build.
- Never edit `negpy/`; `negpy_core` stays its own workspace.
- Subject <=50 chars, body wrapped at 72, no trailing period; split on "and".
- Anti-loop: 10 failed fixes => STOP and ask for human guidance.
- D2 tokens use canonical JSON, not Python-interchangeable.
- D6: `fff_loader` returns None for SGI LogLuv until RLE/tables land.
- D6: JXL fixtures use `imagecodecs`; 16-bit stays `source_u16`.
- Retouch tokens use canonical JSON and SHA-256, not Python repr and SHA-1.
- Clippy `-D warnings` still fails on existing storage/rolls code.
- `skew_fit_side` is private in `negpy_core`; skew detection needs a `core:` promotion.
