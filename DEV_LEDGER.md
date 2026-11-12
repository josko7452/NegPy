# Development Ledger
**Last Updated:** 2026-11-12 19:35 | **Last Commit:** `c2b48082 backend: explain High Efficiency NEF`

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up.
**Source of truth: `negpy_egui/AGENTS.md`**; `MIGRATION_PLAN.md` is advisory.
Phases A–C, D1–D5 and D6 start are complete. D6 RAW decoding is unblocked;
next is camera RAW support.

## Completed (Recent)
- A–C: workspace, core audit, event contract and render worker.
- D1–D3: system leaves, config DTOs, migrations, shipped config and TOML cache.
- D4–D5: storage, sidecars, presets, rolls, composites and analysis.
- D6: constants, source kind, preview memory, JPEG XL boxes and metadata helpers.
- D6: `ImageBuffer` and source encoding (ICC/TRC, untagged rules, gamut fit).
- D6: shared TIFF walker; Coolscan NEF and Flextight FFF detection.
- D6: TIFF/JPEG decoders, IR sidecars and the positive decode.
- `c2b48082` D6: HE NEF reason check reads compressed SubIFD strip heads.

## Next Steps (Ordered)
1. [ ] D6 loaders: camera RAW support behind `RawDecoder`
2. [ ] D6 loaders: JXL, Pakon and Noritsu decoders
3. [ ] D6 loaders: Coolscan NEF and Flextight FFF image decoding
4. [ ] D6 loaders: loader factory wiring

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
- D6: `source_kind` selects TIFF/JPEG/JXL/Pakon/Noritsu; Coolscan NEF and
  Flextight FFF need one shared TIFF walker.
- D6: `tiff_walker::read_ifds` reads every reachable IFD; SubIFDs are marked so
  NEF and FFF rules can tell them from top-level directories.
- D6: TIFF keeps uint16 samples when it is already linear; an untagged 8-bit
  image decodes as sRGB and a profile supplies the positive decode.
- D6: HE NEF check reads SubIFDs directly; `tiff::Decoder` needs top-level
  dimensions absent from the synthetic test NEF.
- D6: HE NEF check mirrors Python: Compression 34713, first StripOffset, then
  a 64-byte `INTOPIX` head.
- Clippy `-D warnings` fails on existing storage/rolls code, so use
  `cargo check --workspace` plus focused tests as the gate.
- D6: `cargo test --workspace` can take ~29 min and grow `target/` heavily;
  the last full run passed.
