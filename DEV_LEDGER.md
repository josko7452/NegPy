# Development Ledger
**Last Updated:** 2026-11-16 02:05 | **Last Commit:** `pending matrix profiles`

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up, backend only. Stop before GUI work.
**Source of truth: `negpy_egui/AGENTS.md`**; `MIGRATION_PLAN.md` is advisory.
User override: 10 commits this session replaces the AGENTS 5-commit stop; targeted tests per commit; workspace test after commit 10.

## Completed (This Session)
- `f0665565` clear-base read cached per source key.
- `11191f6e` Contrast Mask and Diffusion planes; analysis-grid replay; mask expansion.
- `32d851dd` cached stage engine over base, exposure, CLAHE and Lab.
- `caa06080` half-frame slices, hash suffix, point remap, diptych join.
- `4e3380da` decode-side flat field, embedded warp and gain slicing.
- `e69c2cdd` frame-merge rules and assembled source labels.
- `1c9e2313` scan filename templates and the 16-bit TIFF writer.
- `df40538f` thumbnail keys and the positive preview.
- `541f5b1d` headless import: decode, orient, flat-field, unmix, detect, base read.
- `2879c2b8` search query parser, matcher and asset facts.
- `pending` crosstalk and sensor profile TOML I/O.
- `f5026088` format; earlier D7-D12 partials in git history.

## Audit (After This Session)
- 181 Rust modules, 1,069 `pub fn`. The D10 engine now assembles base, planes, exposure and tail
  with checkpoints; no GPU engine or `preview_manager` exists.
- **D11:** frame merge lacks the TIFF write and verify; contact-sheet painting and
  the Linear Output per-format decoders remain.
- **D12:** embeddings, semantic model, gear match, rehome, naming, triplets, the
  remaining migrations and half-frame detection remain.
- **D13/UI:** session glue, controller and every egui view (out of scope).

## Next Steps (Ordered)
1. [ ] D11: finish the remaining Linear Output decoders (RAW/Pakon/Noritsu/JXL descriptors).
2. [ ] D12: embeddings, semantic model, gear match, rehome, naming, the remaining migrations.
3. [ ] D10: GPU engine, preview manager and `services.view.coordinate_mapping`.
4. [ ] capture/scanner backends and `infrastructure.display`; then D13/UI.

## Constraints / Gotchas
- Workspace test after 10 commits: `CARGO_BUILD_JOBS=2 cargo test --workspace --no-fail-fast`
  passed (no failures; one pre-existing unused import warning). `cargo fmt --check` clean.
- Never edit `negpy/`; `negpy_core` stays its own workspace. `negpy_egui` gained `md5`,
  `ureq`, `minijinja` and `chrono`.
- Fingerprint and export bytes are pinned to Python. A change that moves them breaks the
  cache and export parity.
- Stitch composite and IR blending are pinned on translation fixtures only.
- Skew synthetic round-trip test blocked: debug and release exceed 60s on a small strip.
- D6: `fff_loader` returns None for SGI LogLuv until RLE/tables land.
- Clippy `-D warnings` still fails on pre-existing storage/export code.
