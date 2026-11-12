# Development Ledger
**Last Updated:** 2026-11-13 05:25 | **Last Commit:** `93d440db backend: port the photometric processor`

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up.
**Source of truth: `negpy_egui/AGENTS.md`**; `MIGRATION_PLAN.md` is advisory.
Phases A–C, D1–D5 and D6 are complete. D7 process-mode detection and the
exposure kernels have landed; processor wiring is next.

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

## Next Steps (Ordered)
1. [ ] D7: wire exposure stages into `DarkroomEngine`

## Constraints / Gotchas
- 5-commit hard stop; update this ledger before every commit. Keep it <60 lines.
- Targeted tests per commit; full `cargo test --workspace` only after commit 5.
- Last full workspace test: not run this session (targeted tests only).
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
