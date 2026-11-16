# Development Ledger
**Last Updated:** 2026-11-16 | **Last Commit:** `3ffd3dc5` kernel: add the shared image conversion helpers

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up, backend only. Stop before GUI work.
**Source of truth: `negpy_egui/AGENTS.md`**; `MIGRATION_PLAN.md` is advisory.
This session ran 10 backend commits, then the workspace test and a gap audit.

## Completed (This Session)
- `889169b8` display: build and apply the 3D ICC LUT (qcms).
- `7401abfe` display: build matrix/TRC transform tables (8- and 16-bit).
- `8c47f220` display: resolve profiles and transform a preview.
- `c5cdf056` assets: implement the CLIP byte-level BPE tokenizer.
- `fce606fd` assets: rank and persist semantic-search vectors.
- `314acf89` assets: carry an assembled frame's edit to its merge.
- `8209186b` system: collect per-step wall time into one line.
- `9d04b3f9` filesystem: scan a watched folder for new sources.
- `5f1de1d2` capture: plan a blend exposure and stage a capture.
- `3ffd3dc5` kernel: add the shared image conversion helpers (10 total).

## Verification (After Commit 10)
- Targeted tests passed per commit.
- `cargo test --workspace --no-fail-fast` passed: 131 test binaries, 0 failures.
- `cargo fmt --all -- --check` clean. Clippy has 3 pre-existing errors and is not
  part of the workspace gate: `stats.rs` LOG10_2, `logluv.rs` M_LN2, and an
  `eq_op` in `tiff_loader.rs`.

## Audit (After Commit 10)
- Covered under another path: `altprocess` stages; feature logic in
  `features/*/processor.rs`; render tail in `exposure_pipeline.rs`.
- **Gap found, not fixed:** `PrintSettings` hardcodes dye separation at identity
  in `print.rs::print_curve`, and `render_negative` never fills it from
  `ExposureConfig`. Python passes `dye_separation`, its trims and
  `separation_damping`. This moves print pixels; fix with a bounds and
  thumbnail version bump.
- **D10:** render engine exists; `image_processor.py`, `preview_manager.py`
  and `gpu_engine.py` remain.
- **D11:** scanner transports remain (`service.py` hardware loop); export
  encoders and contact-sheet have backend paint.
- **D12:** monitor profile detection (colord/CoreGraphics), ONNX inference,
  gear library save, and capture transports remain. `semantic_model.py` now
  has ranking and preprocessing but no session runner.
- **D13/UI:** session glue, controller and every egui view stay out of scope.

## Next Steps (Ordered)
1. [ ] Print dye separation and its damping in `render_negative`, with a
   `BOUNDS_VERSION` and `THUMBNAIL_RENDER_VERSION` bump.
2. [ ] D12: monitor profile detection and gear library save.
3. [ ] D12: ONNX session runner over the new CLIP tokenizer and ranking.
4. [ ] D10: image processor and preview manager (backend worker).
5. [ ] D11: scanner transport adapters over the write policy.

## Constraints / Gotchas
- Never edit `negpy/`; `negpy_core` stays its own workspace.
- Fingerprint and export bytes are pinned to Python.
- `negpy-backend` gained `qcms`, `html-escape` and `regex`.
- `negpy_egui/target` reached 126 GB. Prune it with
  `cargo clean --profile dev` (keeps release) or `cargo clean -p <crate>`;
  do not wipe the whole directory.
- The workspace test links 127 integration binaries; on a loaded host it is slow.
- D6: `fff_loader` returns None for SGI LogLuv until RLE/tables land.
