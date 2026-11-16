# Development Ledger
**Last Updated:** 2026-11-16 | **Last Commit:** `capture: store the scan recipes`

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up, backend only. Stop before GUI work.
**Source of truth: `negpy_egui/AGENTS.md`**; `MIGRATION_PLAN.md` is advisory.
This session runs 10 commits, then the workspace test and a new gap audit.

## Completed (This Session)
- `chart` reference Lab and CIEDE2000 with the published-pair oracle.
- `chart` corner-patch interpolation and four-way grid orientation.
- `chart` Levenberg-Marquardt fit with the unit diagonal and ridge.
- `capture` shutter labels, ladder spacing, display and start point.
- `capture` scan presets and framing levels over the settings store.
- `a82c3ae2` view: viewport/raw coordinate maps, off-frame homography.
- `a6206a94` export: Pakon model descriptions, XMP provenance, TIFF datetime.
- `4072373e` film-edge: DX and KEYKODE barcode bits.
- `269eccba` film-edge: maker run layouts for 135 and 120.
- `3969e0b0` gear: film edge block, `printsAs`, bundled flag.
- `36e68768` film-edge: edge style and item generation.
- `c6e2ec4a` export: contact-sheet palette, paper black and DX flag.
- `74a01b4f` metadata: EXIF formatting, scan record, XMP rating.
- `d819ae70` metadata: export payload with flags and preview rows.
- `fce2f98d` metadata: XMP packet builder.

## Verification (After Commit 10)
- `cargo fmt --check` clean.
- `CARGO_BUILD_JOBS=2 cargo test --workspace --no-fail-fast` passed.
  First run failed to link (`ld` bus error) on a full disk; `cargo clean`
  freed 159 GB and the rerun passed.
- `cargo clippy -p negpy-backend --lib` still has 3 pre-existing errors:
  `exposure/stats.rs` LOG10_2, `loaders/logluv.rs` M_LN2,
  `loaders/tiff_loader.rs` `255.0 / 255.0`. Not touched this session.

## Audit (After Commit 10)
- Closed this session: coordinate mapping, Linear Output descriptors/XMP,
  film-edge barcodes/layouts/style/items, gear edge data, sheet look,
  EXIF read, metadata payload, XMP write.
- **D11:** film-edge painter (`services/film_edge/paint.py`) still absent:
  it needs a text glyph source without a GUI dependency. Contact-sheet
  painting and `services/scanning/service.py` remain.
- **D12:** embeddings, semantic model, CLIP tokenizer, calibration/meter,
  capture presets/service, preview manager, and `infrastructure/display/*`.
- **D10:** GPU engine and `services/rendering/{engine,image_processor}` remain.
- **D8/D9:** `features/process/chart.py` lacks ΔE00, orientation detection
  and the matrix fit. `retouch/clone.py` delegates to the already-ported logic.
- **D13/UI:** session glue, controller and every egui view (out of scope).

## Next Steps (Ordered)
1. [ ] D11: film-edge paint (glyph source), contact-sheet paint.
2. [ ] D11: scanning service over the scanner backends.
3. [ ] D11: film-edge paint (glyph source), contact-sheet paint.
4. [ ] D11: scanning service over the scanner backends.
5. [ ] D12: capture calibration, meter and presets.
6. [ ] D10: image processor and GPU engine gaps.

## Constraints / Gotchas
- Never edit `negpy/`; `negpy_core` stays its own workspace.
- Fingerprint and export bytes are pinned to Python.
- `negpy-backend` gained `unicode-normalization` for the edge ASCII fold.
- `negpy_egui/target` reached 157 GB and broke the linker. Prune it with
  `cargo clean --profile dev` (keeps release) or `cargo clean -p <crate>`;
  do not wipe the whole directory.
- Skew synthetic round-trip test still blocked on runtime.
- D6: `fff_loader` returns None for SGI LogLuv until RLE/tables land.
