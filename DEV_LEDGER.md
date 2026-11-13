# Development Ledger
**Last Updated:** 2026-11-13 13:20 | **Last Commit:** `578fd82a backend: route slide density stages`

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up.
**Source of truth: `negpy_egui/AGENTS.md`**; `MIGRATION_PLAN.md` is advisory.
User override: 10 commits this session; targeted tests per commit; workspace test after commit 10.
**Audit rule:** a merged Rust file is not parity proof; the gaps below remain.

## Audit (After Commit 10)
- Public Python callables: 1674; Rust `pub fn`: 775; exact-name gaps: 860.
- D9: transparency curve, density stage, WB/Kelvin and pick-WB landed; `fade_white_grid`, `density_stage_grid` and `measure_transfer_neutral_axis` remain.
- D9: lens warps apply through `LensMetadata::apply`, not the Python `apply_lens` name; Panasonic/Sony fill-chain parity is partial.
- D9: stitch helpers exist; full `stitch_composite` parity and IR composite tests remain.
- D8: batch Auto Crop, film-border measurement, rebate helpers and `rebate_tone` remain.
- D7: fade estimate orchestration, auto-red hints and analysis readouts remain.
- D12: gear-preset conversion exists; startup wiring and asset/session glue remain.
- D10-D11, D13/E1-E8: engine wiring, export/scanning, desktop glue and egui UI remain.

## Completed (This Session)
- `578fd82a` D9: slide density stages and WB/Kelvin helpers.
- `4b33cb3a` D9: stitch registration and blending.
- `9066781d` D9: embedded lens warps.
- `39492533` D9: RGB-scan grouping and assembly.
- `92eaf76c` D9: slide transfer curve.
- `7d41c1df` D8: frame-skew detection.
- `0ec19b06` D7: zone placement.
- `aac84a2d` D7: Restore Range and Auto Red.
- `3569f4de` D12: gear-preset migration.

## Next Steps (Ordered)
1. [ ] D9: port transparency `fade_white_grid`, `density_stage_grid` and neutral-axis helpers.
2. [ ] D9: expose `apply_lens` and pin Panasonic/Sony fill-chain parity.
3. [ ] D9: pin full `stitch_composite` and IR composite parity.
4. [ ] D8: port batch Auto Crop, film-border measurement and rebate helpers.
5. [ ] D7: port fade estimate orchestration and remaining analysis readouts.

## Constraints / Gotchas
- Full workspace test: 2026-11-13 13:15, passed with `CARGO_BUILD_JOBS=2 cargo test --workspace --no-fail-fast`; pre-final-commit state was green.
- Skew synthetic round-trip test blocked: debug and release exceed 60s on a small strip; only abstain cases are pinned.
- Stitch composite and IR blending compile, but only acylindrical helpers are pinned.
- Never edit `negpy/`; `negpy_core` stays its own workspace.
- Subject <=50 chars, body wrapped at 72, no trailing period; split on "and".
- Anti-loop: 10 failed fixes => STOP and ask for human guidance.
- D2 tokens use canonical JSON, not Python-interchangeable.
- D6: `fff_loader` returns None for SGI LogLuv until RLE/tables land.
- D6: JXL fixtures use `imagecodecs`; 16-bit stays `source_u16`.
- Clippy `-D warnings` still fails on existing storage/rolls code.
