# Development Ledger
**Last Updated:** 2026-11-16 | **Last Commit:** `4e4b67ad` backend: apply print dye separation and damping

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up, backend only. Stop before GUI work.
**Source of truth: `negpy_egui/AGENTS.md`**; `MIGRATION_PLAN.md` is advisory.
This session runs 10 backend commits, then the workspace test and a gap audit.

## Completed (This Session)
- `3ffd3dc5` kernel: add the shared image conversion helpers.
- `4e4b67ad` backend: apply print dye separation and damping.

## Verification (After Commit 10)
- Prior session: 131 test binaries passed.
- Targeted tests run per commit. Workspace tests run after commit 10.

## Audit (After Commit 10)
- The prior audit's version-bump note was wrong: Python already renders these
  controls at the current bounds and thumbnail versions.
- **D10:** render engine exists; `image_processor.py`, `preview_manager.py`
  and `gpu_engine.py` remain.
- **D11:** scanner transports remain (`service.py` hardware loop); export
  encoders and contact-sheet have backend paint.
- **D12:** monitor profile detection (colord/CoreGraphics), ONNX inference,
  gear library save, and capture transports remain. `semantic_model.py` now
  has ranking and preprocessing but no session runner.
- **D13/UI:** session glue, controller and every egui view stay out of scope.

## Next Steps (Ordered)
1. [ ] Print dye separation and its damping in `render_negative`.
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
