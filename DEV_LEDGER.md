# Development Ledger
**Last Updated:** 2026-11-13 00:05 | **Last Commit:** `pending lab`

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up.
**Source of truth: `negpy_egui/AGENTS.md`**; `MIGRATION_PLAN.md` is advisory.
Phases A–C, D1–D5 and D6 are complete. D7 is complete. D8 ports the
geometry, lab, local, retouch, toning and finish kernels over ten commits.

## Completed (Recent)
- A–C and D1–D5 complete: workspace, event contract, storage and assets.
- D6: constants, source kind, metadata, TIFF/JPEG and source encoding.
- D6: camera RAW, JPEG XL, Pakon, Noritsu, Coolscan and Flextight decode.
- D6: source selection and one `factory::load` dispatch.
- D7: process-mode detection ports the Python crop, stats and thresholds.
- D7: B&W collapse, filter weights and printed-exposure filtering.
- D7: log-density bounds, ROI metering and luma/color mixing.
- D7: paper registry, tonal overrides and density matrices.
- D7: grade, pivot, shadow/highlight holds and the print wrapper.
- D7: preflash parameters and value math.
- D7: exposure metering, analysis regions and crosstalk unmix.
- D7: normalization orchestration, bound resolution and point trims.
- D7: photometric print curve, split grade and B&W collapse.
- D7: NPZ print-LUT loading through `zip` and `npyz`.
- D7: exposure pipeline entry point normalizes then prints a frame.
- D8: shared Adobe RGB Lab spaces, gamma and clip helper.
- D8: Lab stage order, sharpening, saturation, denoise, glow and CLAHE.

## Next Steps (Ordered)
1. [ ] D8: port chemical and split toning
3. [ ] D8: port finish vignette and carrier
4. [ ] D8: port geometry transforms and crops
5. [ ] D8: port geometry detection
6. [ ] D8: port local masks
7. [ ] D8: port retouch detection and IR
8. [ ] D8: port retouch repair and clone
9. [ ] D8: wire the D8 stage order and verify

## Constraints / Gotchas
- 5-commit hard stop; update this ledger before every commit. Keep it <60 lines.
- Targeted tests per commit; full `cargo test --workspace` only after commit 5.
- Last full workspace test: 2026-11-12 23:04, passed with `CARGO_BUILD_JOBS=1`.
- Never edit `negpy/`; `pub` promotion in `negpy_core/src/*.rs` needs human approval.
- `negpy_core` stays its own workspace; no root `Cargo.toml`.
- No new crates can be fetched here. `git` writes need escalation.
- Subject ≤50 chars, body wrapped at 72, no trailing period; split on "and".
- Anti-loop: 3 failed fixes ⇒ STOP and ask for human guidance.
- `apply_patch` paths resolve against `negpy_egui/`, not the repo root.
- `.git/info/exclude` ignores `AGENTS.md`; `DEV_LEDGER.md` is force-added.
- D1: i18n embeds `../negpy/translations/` with `include_str!`; no copy.
- D2: composite tokens use canonical JSON, stable here but not Python-interchangeable.
- D2: flat `to_dict` emits JSON in section order; Python uses `json.dumps` defaults.
- D5: roll records stay flat JSON; only modeled sections take resolved defaults.
- D6: TIFF keeps uint16 samples when already linear; untagged 8-bit is sRGB.
- D6: HE NEF check reads Compression 34713, StripOffset and an `INTOPIX` head.
- D6: `RawlerDecoder` keeps sensor RGB; as-shot WB only when valid.
- D6: JXL fixtures were written with `imagecodecs`; 16-bit stays `source_u16`.
- D6: `fff_loader::load` returns None for SGI LogLuv until RLE/tables land.
- Clippy `-D warnings` fails on existing storage/rolls code, so use
  `cargo check --workspace` plus focused tests as the gate.
- Full `cargo test --workspace` is slow because it links every test binary;
  the last full run passed.
