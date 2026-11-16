# Development Ledger
**Last Updated:** 2026-11-16 | **Last Commit:** `metadata: read the container resolution`

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up, backend only. Stop before GUI work.
**Source of truth: `negpy_egui/AGENTS.md`**; `MIGRATION_PLAN.md` is advisory.
This session adds 10 backend commits, then runs the workspace test and a gap audit.

## Completed (This Session)
- `pending` metadata: custom EXIF tags and exposure parsing.
- `pending` metadata: source EXIF cleanup and thumbnail removal.
- `pending` metadata: capture merge over source EXIF.

## Verification (After Commit 10)
- `cargo fmt --check` clean. Targeted tests passed per commit.
- `cargo test --workspace --no-fail-fast` passed after commit 10.
- Clippy remains red on pre-existing lints across `negpy-backend`; no lint
  sweep was folded into these feature commits.
- The full workspace build took 17m on first run because every integration
  test linked in parallel. Disk is healthy after the earlier cleanup.

## Audit (After Commit 10)
- Closed this session: chart reference/ΔE/orientation/LM fit, shutter label
  parsing and start point, capture presets, Pick Better, filesystem dates,
  EXIF and container resolution, matrix/TRC ICC parsing.
- **D11:** `film_edge/paint.py` still needs a glyph source; contact-sheet
  painting and `scanning/service.py` remain.
- **D12:** capture meter/service and transports, preview manager,
  embeddings/semantic/CLIP, monitor profile and ICC LUT application remain.
- **D10:** `rendering/image_processor.py` and `gpu_engine.py` remain.
- **D8/D9:** `metadata/writer.py` remains the largest export gap;
  `retouch/clone.py` delegates to the ported logic.
- **D13/UI:** session glue, controller and every egui view stay out of scope.

## Next Steps (Ordered)
1. [ ] D11: film-edge paint (glyph source), contact-sheet paint.
2. [ ] D11: scanning service over the scanner backends.
3. [ ] D12: capture meter/service and presets wiring.
4. [ ] D12: preview manager, embeddings/semantic/CLIP.
5. [ ] D8/D9: metadata writer.
6. [ ] D10: image processor and GPU engine.

## Constraints / Gotchas
- Never edit `negpy/`; `negpy_core` stays its own workspace.
- Fingerprint and export bytes are pinned to Python.
- `negpy-backend` gained `unicode-normalization` for the edge ASCII fold.
- `negpy_egui/target` reached 157 GB and broke the linker. Prune it with
  `cargo clean --profile dev` (keeps release) or `cargo clean -p <crate>`;
  do not wipe the whole directory.
- Skew synthetic round-trip test still blocked on runtime.
- D6: `fff_loader` returns None for SGI LogLuv until RLE/tables land.
