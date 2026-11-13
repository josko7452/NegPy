# Development Ledger
**Last Updated:** 2026-11-13 09:55 | **Last Commit:** `e44fa87e backend: zero fade crossover on the gray midtone`

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up.
**Source of truth: `negpy_egui/AGENTS.md`**; `MIGRATION_PLAN.md` is advisory.
User override: 10 commits this session; targeted tests per commit, workspace test after commit 10.
**Audit rule:** a merged Rust file is not proof of parity; symbol-level gaps below remain.

## Audit (After Commit 10)
- Public Python callables: 1102; identifier-missing 590. Rust files alone are not parity proof.
- D12: `roll_fields` lacks `with_frame_seed`/`_with_roll_crop`; gear-preset migration absent; thumbnails, embeddings, half-frame, gear assets, capture/view/session glue absent.
- D7: `placement` absent; exposure logic lacks characteristic-curve application, auto shadow/highlight, WB/Kelvin helpers; analysis strip/ring/vectorscope helpers absent; fade estimate, auto-red and range helpers absent.
- D8: geometry `skew`, `batch_autocrop`, film-border/edges, `rebate_mask` absent; `rebate_tone` absent.
- D9: lens, RGB scan, stitch, transparency, cyanotype, lith, sabattier, pick-better absent.
- D10: CPU/GPU engine wiring, display/color management, WGSL shaders and parity tests absent.
- D11: export, scanning, film-edge and templating absent. D13/E1-E8: desktop glue and egui UI absent.
- D6: `read_exif_from_file`, bounded previews, `NonStandardFileWrapper`, supported demosaic list and lens bind absent.

## Completed (This Session)
- `e44fa87e` D7: zeros the crossover table on the gray midtone.
- `cee6eb42` D7: shares the camera white-balance fold.
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
- `54b34fb6` D9: ports HDR merge path.

## Next Steps (Ordered)
1. [ ] D12: port the gear-preset migration.
2. [ ] D7: add fade range and auto-red helpers after transparency.
3. [ ] D7: port exposure `placement` and the remaining fade/estimate helpers.
4. [ ] D8: port skew detection, batch Auto Crop and film-border helpers.
5. [ ] D9: port lens, RGB scan, stitch and transparency.

## Constraints / Gotchas
- Full workspace test: 2026-11-13 09:50, passed with `CARGO_BUILD_JOBS=2 cargo test --workspace --no-fail-fast` in 1m55s.
- Never edit `negpy/`; `negpy_core` stays its own workspace.
- Subject <=50 chars, body wrapped at 72, no trailing period; split on "and".
- Anti-loop: 10 failed fixes => STOP and ask for human guidance.
- D2 tokens use canonical JSON, not Python-interchangeable.
- D6: `fff_loader` returns None for SGI LogLuv until RLE/tables land.
- D6: JXL fixtures use `imagecodecs`; 16-bit stays `source_u16`.
- Retouch tokens use canonical JSON and SHA-256, not Python repr and SHA-1.
- Clippy `-D warnings` still fails on existing storage/rolls code.
- `skew_fit_side` is private in `negpy_core`; skew detection needs a `core:` promotion.
