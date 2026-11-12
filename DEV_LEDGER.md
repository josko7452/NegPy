# Development Ledger
**Last Updated:** 2026-11-12 23:55 | **Last Commit:** `7207a476 backend: decode Flextight LogLuv strips`

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up.
**Source of truth: `negpy_egui/AGENTS.md`**; `MIGRATION_PLAN.md` is advisory.
Phases A–C, D1–D5 and D6 loaders are complete except factory dispatch.
Next is a single loader entry point over the format loaders.

## Completed (Recent)
- A–C and D1–D5 complete: workspace, event contract, storage and assets.
- D6: constants, source kind, metadata, TIFF/JPEG and source encoding.
- D6: camera RAW decodes through `rawler` behind `RawDecoder`.
- D6: JPEG XL decodes 8-bit floats and keeps 16-bit samples.
- D6: Pakon decodes planar or interleaved samples into RGB16.
- D6: Noritsu decodes chunky BGR16 into RGB16.
- D6: Coolscan subifds and uncompressed FFF pages decode.
- D6: LogLuv tables, RLE strips and RGB conversion are in `logluv.rs`.
- D6: Flextight LogLuv strips decode and normalize to RGB.
- D6: Source selection tells Coolscan NEF and Flextight FFF apart.

## Next Steps (Ordered)
1. [ ] D6 loaders: factory dispatch
2. [ ] D7: process feature kernels

## Constraints / Gotchas
- 5-commit hard stop; update this ledger before every commit. Keep it <60 lines.
- Tests from `negpy_egui/`: `cargo check --workspace && cargo test --workspace`.
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
- D5: `edited_file_names_under` returns a `Vec`; callers compare as sets.
- D5: roll records stay flat JSON; only modeled sections take resolved defaults.
- D6: `tiff_walker::read_ifds` marks SubIFDs; keep top-level and SubIFD rules
  separate for NEF and FFF.
- D6: TIFF keeps uint16 samples when already linear; untagged 8-bit is sRGB.
- D6: HE NEF check reads Compression 34713, StripOffset and an `INTOPIX` head.
- D6: `RawlerDecoder` keeps sensor RGB; apply as-shot WB only when valid and
  carry `rgb_xyz_matrix` to the transparency transfer.
- D6: JXL fixtures were written with `imagecodecs`; the loader keeps 8-bit
  samples and stores 16-bit as `source_u16`.
- D6: `fff_loader::load` returns None for SGI LogLuv until the RLE and
  tables are ported; `negpy_core::loaders` already holds the primitives.
- Clippy `-D warnings` fails on existing storage/rolls code, so use
  `cargo check --workspace` plus focused tests as the gate.
- D6: `cargo test --workspace` can take ~29 min and grow `target/` heavily;
  the last full run passed.
