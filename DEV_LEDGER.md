# Development Ledger
**Last Updated:** 2026-11-13 05:10 | **Last Commit:** `fc086714 backend: wire the D8 tail stages`

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up.
**Source of truth: `negpy_egui/AGENTS.md`**; `MIGRATION_PLAN.md` is advisory.
User override: this session runs 10 commits, focused on the missing D12 asset migrations.

## Completed (Recent)
- D12: ported the bounds, Cast Removal and B&W source-color row sweeps.
- D12: added the shared edit-row rewrite helper and done-flag handling.
- D6: source kinds, metadata and TIFF/JPEG/JXL/RAW/Pakon/Noritsu/FFF decode.
- D7: detection, normalization, curves, metering, papers and print.
- D8: Lab, toning, finish, local, retouch, single-frame geometry and tail order.

## Next Steps (Ordered)
1. [ ] D12: port the legacy hash rehome and ambiguous-hash helpers.
2. [ ] D12: port the normalization-roll and Positive auto-meter migrations.
3. [ ] D12: port the roll card split, lock and section-push migrations.
4. [ ] D12: port the Cast Removal and Crop moves to frame seeds.
5. [ ] D12: port the Film card source-encoding migration.
6. [ ] D12: port the legacy flat-field profile migration.
7. [ ] D12: port the flat-field sensor-layout migration.
8. [ ] D12: port the gear-preset migration.
9. [ ] D7: port the missing exposure and process groups.
10. [ ] D8: port geometry skew and batch auto-crop.

## Constraints / Gotchas
- User override 2026-11-13: 10 commits this session; targeted tests per commit, one workspace test at the end.
- Last full workspace test: 2026-11-13 04:00, passed with `CARGO_BUILD_JOBS=1`.
- Never edit `negpy/`; `pub` promotion in `negpy_core/src/*.rs` needs human approval.
- `negpy_core` stays its own workspace; no root `Cargo.toml`.
- Subject ≤50 chars, body wrapped at 72, no trailing period; split on "and".
- Anti-loop: 3 failed fixes ⇒ STOP and ask for human guidance.
- `apply_patch` paths resolve against `negpy_egui/`, not the repo root.
- `.git/info/exclude` ignores `AGENTS.md`; `DEV_LEDGER.md` is force-added.
- D1: i18n embeds `../negpy/translations/` with `include_str!`; no copy.
- D2: composite tokens use canonical JSON, not Python-interchangeable.
- D5: roll records stay flat JSON; only modeled sections get defaults.
- D6: TIFF keeps uint16 samples when linear; untagged 8-bit is sRGB.
- D6: JXL fixtures use `imagecodecs`; 16-bit stays `source_u16`.
- D6: `fff_loader::load` returns None for SGI LogLuv until RLE/tables land.
- Clippy `-D warnings` still fails on existing storage/rolls code.
