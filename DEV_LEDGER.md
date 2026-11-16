# Development Ledger
**Last Updated:** 2026-11-16 | **Last Commit:** `4fb5d762` metadata: reject a spaced EV sum

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up, backend only. Stop before GUI work.
**Source of truth: `negpy_egui/AGENTS.md`**; `MIGRATION_PLAN.md` is advisory.
This session ran 10 backend commits, then the workspace test and a gap audit.

## Completed (This Session)
- `63609792` metadata: custom EXIF tags and exposure parsing.
- `54fbbdf7` metadata: source EXIF cleanup and thumbnail removal.
- `8244431f` metadata: capture merge over source EXIF.
- `6eca2b74` metadata: JPEG EXIF and XMP segments.
- `94e80e16` metadata: PNG, WebP and JXL metadata rewrite.
- `a2151c03` film-edge: fixed face and measured text runs.
- `2a3e7543` film-edge: strip rebate, marks and perforations.
- `1fc03b35` contact-sheet: tile, strip, row and label rendering.
- `d7aa35ec` capture: linear probe meter and shutter recommendation.
- `a6ee0fb4` scanning: name, collision and write policy.
- `4fb5d762` metadata: reject a spaced EV sum (post-10 test fix).

## Verification (After Commit 10)
- Targeted tests passed per commit.
- `cargo test --workspace --no-fail-fast` passed. First post-10 run exposed
  one EV-parser unit failure; fixed in `4fb5d762`, then the rerun passed.
- `cargo fmt --check` clean. Clippy is not part of this check.

## Audit (After Commit 10)
- Closed this session: D8/D9 metadata writer, D11 film-edge paint,
  contact-sheet paint, scanning write policy, D12 capture meter core.
- Covered under another path: `altprocess` stages, feature logic in
  `features/*/processor.rs`, render tail in `exposure_pipeline.rs`.
- **D10:** render engine exists; `image_processor.py`, `preview_manager.py`
  and `gpu_engine.py` remain.
- **D11:** scanner transports remain (`service.py` hardware loop); export
  encoders and contact-sheet now have backend paint.
- **D12:** embeddings/semantic/CLIP, monitor profile and ICC LUT application,
  and capture transports remain.
- **D13/UI:** session glue, controller and every egui view stay out of scope.

## Next Steps (Ordered)
1. [ ] D12: ICC LUT application and monitor profile.
2. [ ] D12: embeddings, semantic model and CLIP tokenizer.
3. [ ] D10: image processor and preview manager (backend worker).
4. [ ] D11: scanner transport adapters over the write policy.
5. [ ] D10: GPU engine once a headless wgpu surface is in place.

## Constraints / Gotchas
- Never edit `negpy/`; `negpy_core` stays its own workspace.
- Fingerprint and export bytes are pinned to Python.
- `negpy-backend` gained `unicode-normalization` for the edge ASCII fold.
- `negpy_egui/target` reached 157 GB and broke the linker. Prune it with
  `cargo clean --profile dev` (keeps release) or `cargo clean -p <crate>`;
  do not wipe the whole directory.
- Skew synthetic round-trip test still blocked on runtime.
- D6: `fff_loader` returns None for SGI LogLuv until RLE/tables land.
