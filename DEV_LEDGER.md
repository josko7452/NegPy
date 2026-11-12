# Development Ledger
**Last Updated:** 2026-11-12 18:54 | **Last Commit:** `pending backend: detect Coolscan NEF and Flextight FFF`

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up.
**Source of truth: `negpy_egui/AGENTS.md`**; `MIGRATION_PLAN.md` is advisory.
Phases A–C, D1–D5 and D6 start are complete. Next is D6 image decoders.

## Completed (Recent)
- A–C: workspace, core audit, event contract and render worker.
- D1–D3: system leaves, config DTOs, migrations, shipped config and TOML cache.
- D4–D5: storage, sidecars, presets, rolls, composites and analysis.
- D6: constants, source kind, preview memory, JPEG XL boxes and metadata helpers.
- D6: `ImageBuffer` and source encoding (ICC/TRC, untagged rules, gamut fit).
- D6: shared TIFF walker; Coolscan NEF and Flextight FFF detection.

## Next Steps (Ordered)
1. [ ] D6 loaders: TIFF and JPEG decoders
2. [ ] D6 loaders: RawDecoder trait and camera RAW
3. [ ] D6 loaders: JXL, Pakon and Noritsu decoders
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
- D6: `cargo test --workspace` can take ~29 min and grow `target/` heavily;
  the last full run passed.
