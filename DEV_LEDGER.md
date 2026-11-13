# Development Ledger
**Last Updated:** 2026-11-13 15:20 | **Last Commit:** `b4f5924e backend: collect crop evidence per frame`

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up.
**Source of truth: `negpy_egui/AGENTS.md`**; `MIGRATION_PLAN.md` is advisory.
User override: 10 commits this session; targeted tests per commit; workspace test after commit 10.
**Audit rule:** a merged Rust file is not parity proof; the gaps below remain.

## Audit (After Commit 10)
- Public Python callables: 1674; Rust `pub fn`: 775; exact-name gaps: 860.
- D9: transparency density grids and the Positive cast window landed; wire them into the render engine.
- D9: `apply_lens` is exposed; Panasonic/Sony fill-chain pixel parity is still unpinned.
- D9: stitch IR blending follows coverage; a real photograph panorama fixture remains.
- D8: batch Auto Crop evidence and resolver landed; `rebate_tone` and the composite wiring remain.
- D7: fade estimate orchestration, auto-red hints and analysis readouts remain.
- D12: gear-preset conversion exists; startup wiring and asset/session glue remain.
- D10-D11, D13/E1-E8: engine wiring, export/scanning, desktop glue and egui UI remain.

## Completed (This Session)
- `b4f5924e` D8: per-frame crop evidence.
- `dc2f7db5` D8: roll crop resolver.
- `daabd314` D8: roll crop templates.
- `91dc1038` D8: film border and rebate helpers.
- `b1336f92` D9: stitch IR uses lit coverage.
- `35b89497` D9: lens chain uses the Python entry point and cubic remap.
- `93755351` D9: Positive cast window and retry.
- `1cc5c80a` D9: slide density grids.
- `578fd82a` D9: slide density stages and WB/Kelvin helpers.
- `4b33cb3a` D9: stitch registration and blending.

## Next Steps (Ordered)
1. [ ] D7: port fade estimate orchestration and remaining analysis readouts.
2. [ ] D8: port `rebate_tone` and wire Auto Crop into the composite path.
3. [ ] D10: wire the density grids into the render engine.
4. [ ] D11: port export and scanning services.
5. [ ] D12: port thumbnail and session glue.

## Constraints / Gotchas
- Full workspace test: 2026-11-13 13:15, passed with `CARGO_BUILD_JOBS=2 cargo test --workspace --no-fail-fast`; pre-final-commit state was green.
- Skew synthetic round-trip test blocked: debug and release exceed 60s on a small strip; only abstain cases are pinned.
- Stitch composite and IR blending are pinned on translation fixtures only.
- Never edit `negpy/`; `negpy_core` stays its own workspace.
- Subject <=50 chars, body wrapped at 72, no trailing period; split on "and".
- Anti-loop: 10 failed fixes => STOP and ask for human guidance.
- D2 tokens use canonical JSON, not Python-interchangeable.
- D6: `fff_loader` returns None for SGI LogLuv until RLE/tables land.
- D6: JXL fixtures use `imagecodecs`; 16-bit stays `source_u16`.
- Clippy `-D warnings` still fails on existing storage/rolls code.
