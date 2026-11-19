# Development Ledger
**Last Updated:** 2026-11-19 23:25 | **Last Commit:** pending desktop: keep the rail state between captures

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
1. [ ] Bind the Export and Metadata cards to the frame edit.
2. [ ] Load the Library rows from the saved roll store.
3. [ ] Revisit the Light Table tile metrics.

## Completed (Recent)
- `HEAD` desktop: close the card-binding batch.
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
- Roll capture: total 0.686, layout 0.705. Frame 0.699 to 0.749.
  The golden manifest has no live source path, so the frame and strip are
  blank and drag the total. The rail card stack renders.

## Constraints / Gotchas
- Never edit `negpy/`; `negpy_core` stays its own workspace.
- Reference captures: `negpy_egui/tests/gui/golden/tall/`.
- Run the app with `cd negpy_egui && cargo run --release --bin negfe`.
- Manual profile `/home/bruno/Dokumenty/NegPy`; roll `vv9`.
- A capture can rasterize mid-scroll and under-score the rail.
- Generated captures stay untracked under `tests/gui/actual/`.
- `negfe` is the manual target; `negfe-capture` is the fixed matrix.
