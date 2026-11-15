# Development Ledger
**Last Updated:** 2026-11-15 23:30 | **Last Commit:** `ebbf7213 export: add the linear output core`

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up, backend only. Stop before GUI work.
**Source of truth: `negpy_egui/AGENTS.md`**; `MIGRATION_PLAN.md` is advisory.
User override: 10 commits this session replaces the AGENTS 5-commit stop; targeted tests per commit; workspace test after commit 10.

## Completed (This Session)
- `f0665565` base: clear-base read cached per source key.
- Contrast Mask and Diffusion planes; analysis-grid geometry replay; mask expansion.
- Cached stage engine over base, exposure, CLAHE and Lab checkpoints.
- Half-frame slice bounds, hash suffix, point remap and diptych join.
- Decode-side flat field, embedded warp and gain slicing.
- Frame-merge rules: part files, merged path, decode params, assembled labels.
- Scan filename templates and the 16-bit TIFF writer with IR sidecars.
- Thumbnail cache keys and the positive preview for the filmstrip.
- Headless import: decode, orient, flat-field, unmix, detect the film, read the base.
- Search query parser and matcher; asset facts from file identity and saved metadata.
- Linear Output source classification, gamma, WB, expansion and 16-bit TIFF core.
- Export filename templates and metadata date helpers.
- Contact-sheet roll ordering, format inference and label text.
- Contact-sheet paper, strip, row and pagination planning.
- Film-edge geometry and the frame-turn rule.
- Maps, tile cache and version helpers.
- Prefetch admission policy and the platform memory probe.
- Preview buffer LRU with entry, byte and full-res slot budgets.
- Source token and MD5 config hash; default hash matches Python.
- Flat-master exposure kernel on the print and slide paths.
- `20079eb6` D11: PNG/JPEG/TIFF encoders; PNG and JPEG bytes match Python.
- `c01207eb` D12: thumbnail fingerprint, hashes match Python byte for byte.
- `e6256cf1` D10: fade Estimate orchestration.
- `b07288ce` D10: exposure stage routes by render path (slide base + transfer).
- `fe17f59e` D10: slide base stage; crossover tag matches Python.
- `4edb0491` D11: paper layout, borders and toned paper white.
- `c059f9e6` D10: alt process runs ahead of toning in the tail.
- `1e7e4f44` D9: lith, cyanotype and Sabattier stages.
- `57ba0f19` D8: rebate tone ramp.
- `918be03c` D7: strip/ring/paper/proof-grid helpers.

## Audit (After Commit 10)
- 170 Rust modules, 1,012 `pub fn`. The Python callable count includes private helpers.
- **D10:** the partials are now ported, but no cached engine assembles base, masks,
  exposure and tail. No GPU engine, contrast-mask plane or diffusion plane exists.
- **D10:** `services.rendering.{base_read,lens}` and `preview_manager` remain missing.
- **D11:** frame merge, scanner writers, contact-sheet painting and the Linear Output
  per-format decoders remain. Film-edge has geometry and frame turns only.
- **D12:** thumbnails, embeddings, semantic model, search, naming, half-frame, triplets,
  frame merge assets and the remaining storage migrations.
- **D13/UI:** session glue, controller and every egui view (out of scope).

## Next Steps (Ordered)
1. [ ] D10: build the cached engine over the stage checkpoints.
2. [ ] D10: port `base_read`, `lens`, contrast-mask and diffusion planes.
3. [ ] D11: port frame merge, scanner writers and the Linear Output decoders.
4. [ ] D12: thumbnails and the remaining asset services.

## Constraints / Gotchas
- Workspace test after commit 10: `CARGO_BUILD_JOBS=2 cargo test --workspace --no-fail-fast`
  passed (107 test binaries, no failures).
- Never edit `negpy/`; `negpy_core` stays its own workspace. `negpy_egui` gained `md5`,
  `ureq`, `minijinja` and `chrono`.
- Fingerprint and export bytes are pinned to Python. A change that moves them breaks the
  cache and export parity.
- Stitch composite and IR blending are pinned on translation fixtures only.
- Skew synthetic round-trip test blocked: debug and release exceed 60s on a small strip.
- D6: `fff_loader` returns None for SGI LogLuv until RLE/tables land.
- Clippy `-D warnings` still fails on pre-existing storage/export code.
