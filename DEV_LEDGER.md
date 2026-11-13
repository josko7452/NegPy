# Development Ledger
**Last Updated:** 2026-11-13 17:05 | **Last Commit:** `ff5b309c backend: add the analysis scope readouts`

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up.
**Source of truth: `negpy_egui/AGENTS.md`**; `MIGRATION_PLAN.md` is advisory.
User override: 10 commits this session; targeted tests per commit; workspace test after commit 10.
Workspace test: `CARGO_BUILD_JOBS=2 cargo test --workspace --no-fail-fast` passed after commit 10.
**Audit rule:** a merged Rust file is not parity proof; the gaps below remain.

## Audit (After Commit 10)
- Public Python callables: 2207; Rust `pub fn`: 1132; exact-name gaps: 1643.
- Most exact-name gaps are private helpers inlined in Rust. Real gaps are listed below.
- D9: density grids, Positive cast window, `apply_lens` and stitch IR blending landed; render wiring remains.
- D9: Panasonic/Sony fill-chain pixel parity is still unpinned.
- D8: batch Auto Crop evidence and resolver landed; `rebate_tone` and the composite wiring remain.
- D7: fade estimate and scope readouts landed; the proof-grid helpers remain.
- D12: gear-preset conversion exists; startup wiring and asset/session glue remain.
- D10-D11, D13/E1-E8: engine wiring, export/scanning, desktop glue and egui UI remain.

## Completed (This Session)
- `ff5b309c` D7: waveform color and vectorscope readouts.
- `e36113d5` D7: fade estimate orchestration.
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
1. [ ] D8: port `rebate_tone` and wire Auto Crop into the composite path.
2. [ ] D10: wire the density grids and fade estimate into the render engine.
3. [ ] D7: port the proof-grid helpers from `analysis.py`.
4. [ ] D11: port export and scanning services.
5. [ ] D12: port thumbnail and session glue.

## Constraints / Gotchas
- Workspace test after commit 10: `CARGO_BUILD_JOBS=2 cargo test --workspace --no-fail-fast` passed.
- Skew synthetic round-trip test blocked: debug and release exceed 60s on a small strip; only abstain cases are pinned.
- Stitch composite and IR blending are pinned on translation fixtures only.
- Never edit `negpy/`; `negpy_core` stays its own workspace.
- Subject <=50 chars, body wrapped at 72, no trailing period; split on "and".
- Anti-loop: 10 failed fixes => STOP and ask for human guidance.
- D2 tokens use canonical JSON, not Python-interchangeable.
- D6: `fff_loader` returns None for SGI LogLuv until RLE/tables land.
- D6: JXL fixtures use `imagecodecs`; 16-bit stays `source_u16`.
- Clippy `-D warnings` still fails on existing storage/rolls code.
