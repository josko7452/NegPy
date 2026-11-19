# Development Ledger
**Last Updated:** 2026-11-19 22:52 | **Last Commit:** `HEAD` desktop: refresh the Library rolls

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up. E5 card wiring is active.

## UX Target (supersedes pixel parity)
The goal is the same workflow, controls, order, grouping and density as NegPy.
Layout score is a diagnostic. Stop when the control is usable and in place.

## Deferred (not planned)
- Film Strip cells, canvas bar segments and Contact Sheet tile metrics.
- Exact font widths, letter spacing and icon rasterization.
- Spacing under 4px, a 1-2px header difference, and the Scan footer clip.

## Next Steps (Ordered)
1. [ ] Add the Roll Setup import of source folders.
2. [ ] Revisit the Light Table tile metrics.

## Completed (Recent)
- `HEAD` desktop: refresh the Library rolls.
- `d1bec40b` desktop: sort the Library rows.
- `89192452` desktop: wire the Library Import menu.
- `5bfe2236` backend: store the proof condition in session settings.
- `c2bb6ed8` backend: keep the proof condition out of the export edit.
- `9be68fcc` desktop: load the saved export and metadata presets.
- `7d7efc33` desktop: bind the Export proof and sidecars.
- `99a8313a` desktop: bind the Export file page to the edit.
- `1b763df1` desktop: bind the Metadata gear to the gear library.
- `3ad300f2` desktop: carry ids in the step combo.
- `ec2a4fb7` desktop: bind the Metadata page to the edit.
- `0311b01e` desktop: filter the Library and count its rows.
- `c4beaa3e` desktop: show the roll detail line from the store.
- `561c7d0c` desktop: load the Library rows from the roll store.
- `51d209af` desktop: keep the rail state between captures.
- `e7ffaaa1` desktop: close the card-binding batch.
- `2f091ced` desktop: bind the Finish page to the edit.
- `67dd9434` desktop: bind the Look page to the edit.
- `10be5d3f` desktop: bind the Exposure page to the edit.
- `50797ebc` desktop: bind the Geometry page to the edit.
- `113a6235` desktop: bind the Roll Setup to the edit.
- `be36372b` desktop: bind the metering cards to the edit.
- `0739975e` desktop: bind Optics to the edit.
- `07dd69e4` desktop: bind Raw Decode to the edit.
- `8a45f489` desktop: load and save the frame edit.
- `2763e8d8` desktop: own the frame edit document.
- `cbd6bdbf` desktop: close the ten-commit batch.
- `5de4ba4f`..`bbea3e1c` desktop: the previous card and view batch.

## Verification
- `cargo test --workspace`, `cargo check --workspace` and
  `cargo fmt --all --check` pass.
- Clippy is blocked by existing lints in `negpy-backend` and older desktop
  modules (`manual_is_multiple_of`, `too_many_arguments`). None are new.
- Full workspace suite, format and check pass.
- Golden matrix with the vv9 source: best total 0.771; four layout scores
  reach 0.80. Light Table 0.697 and Contact Sheet 0.726 remain below target.

## Constraints / Gotchas
- Never edit `negpy/`; `negpy_core` stays its own workspace.
- Reference captures: `negpy_egui/tests/gui/golden/tall/`.
- Run the app with `cd negpy_egui && cargo run --release --bin negfe`.
- Manual profile `/home/bruno/Dokumenty/NegPy`; roll `vv9`.
- A capture can rasterize mid-scroll and under-score the rail.
- Generated captures stay untracked under `tests/gui/actual/`.
- `negfe` is the manual target; `negfe-capture` is the fixed matrix.
