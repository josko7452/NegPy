# Development Ledger
**Last Updated:** 2026-11-15 19:58 | **Last Commit:** `20079eb6 export: add the container encoders`

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up, backend only. Stop before GUI work.
**Source of truth: `negpy_egui/AGENTS.md`**; `MIGRATION_PLAN.md` is advisory.
User override: 10 commits this session; targeted tests per commit; workspace test after commit 10.

## Completed (This Session)
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
- 152 Rust modules, 918 `pub fn`. Public Python callables outnumber these because many
  private helpers are inlined; the gaps below are the real missing behavior.
- **Flat master** (`apply_flat_curve`, `_process_flat`) is not ported. The engine's flat
  intent exits before the flat curve on both paths.
- **Kernels without a Rust wrapper:** `kernel.caching.{logic,manager}`, `kernel.image.logic`,
  `services.rendering.{source_identity,prefetch_policy,preview_cache,base_read,lens}`,
  `services.maps`. These gate D10's stage caching and source identity.
- **D10:** no render engine assembling base/exposure/tail with stage caching, no GPU engine,
  no contrast-mask or diffusion planes.
- **D11:** contact-sheet layout/roll, templating, linear output, frame merge, scanner
  writers. The film-edge service has no Rust module; only the geometry edge helpers exist.
- **D12:** thumbnails, embeddings, semantic model, search, naming, gear, half-frame, triplets,
  frame merge assets, and the remaining storage migrations.
- **D13/UI:** session glue, controller, and every egui view (out of scope this session).

## Next Steps (Ordered)
1. [ ] D10: build the cached engine over the stage checkpoints.
2. [ ] D11: port contact-sheet roll reads.
3. [ ] D11: contact sheet, templating and linear output.
4. [ ] D12: thumbnails and the remaining asset services.

## Constraints / Gotchas
- Workspace test after commit 10: `CARGO_BUILD_JOBS=2 cargo test --workspace --no-fail-fast`
  passed (101 test binaries, no failures).
- Never edit `negpy/`; `negpy_core` stays its own workspace. `negpy_egui` gained direct
  `crc32fast` and `flate2` deps this session.
- Fingerprint and export bytes are pinned to Python. A change that moves them breaks the
  cache and export parity.
- Stitch composite and IR blending are pinned on translation fixtures only.
- Skew synthetic round-trip test blocked: debug and release exceed 60s on a small strip.
- D6: `fff_loader` returns None for SGI LogLuv until RLE/tables land.
- Clippy `-D warnings` still fails on existing storage/rolls code.
