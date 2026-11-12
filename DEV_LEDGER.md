# Development Ledger
**Last Updated:** 2026-11-13 04:00 | **Last Commit:** `HEAD backend: wire the D8 tail stages`

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up.
**Source of truth: `negpy_egui/AGENTS.md`**; `MIGRATION_PLAN.md` is advisory.
A–C, D1–D7 and D8 are complete. D9 feature kernels are next.

## Completed (Recent)
- D6: source kinds, metadata, TIFF/JPEG/JXL/RAW/Pakon/Noritsu/FFF decode.
- D7: process detection, exposure normalization, curves, LUTs and print.
- D8: shared Adobe RGB Lab spaces, gamma and clip helper.
- D8: Lab, CLAHE, sharpening, saturation, glow and halation.
- D8: chemical and split toning, including lith overrides.
- D8: finish vignette, carrier profiles and filed carrier.
- D8: geometry transforms, crop resolution and coordinate mapping.
- D8: film detection, opaque-border trim, auto-crop and ratio snapping.
- D8: local masks, limited planes, overlap and geometry mapping.
- D8: dust/IR detection, scratch tracing and source tokens.
- D8: exclusion release, score repair, inpaint and clone strokes.
- D8: CLAHE → Lab → toning → crop → finish tail order with Flat bypass.

## Next Steps (Ordered)
1. [ ] D9: port flatfield, HDR, lens, RGB scan, stitch and slide kernels.
2. [ ] D9: port transparency, alt-process, metadata and pick-better kernels.
3. [ ] D9: verify CPU/GPU parity and source-bake order.

## Constraints / Gotchas
- User override 2026-11-13: this session ran 10 commits for the D8 scope.
- Last full workspace test: 2026-11-13 04:00, passed with `CARGO_BUILD_JOBS=1`.
- `cargo check --workspace` also passes. Use it plus focused tests for each commit.
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
