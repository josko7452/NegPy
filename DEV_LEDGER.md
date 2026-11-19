# Development Ledger
**Last Updated:** 2026-11-19 21:35 | **Last Commit:** `HEAD` desktop: bind Raw Decode to the edit

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up. E5 card wiring is active.

## UX Target (supersedes pixel parity)
The goal is the same workflow, controls, order, grouping and density as
NegPy. It is not pixel identity. Layout score is a diagnostic, not a gate:
use it to catch a region that collapsed or moved, not to chase a few pixels.
Stop a polish task when the control is usable and in the right place. Defer
anything whose only gain is a closer score.

## Deferred (not planned)
- Film Strip cell ratios, film-band inset and scrollbar thickness.
- Canvas bar segment widths and icon glyph shapes.
- Contact Sheet tile size and the 1080x760 dialog metrics.
- Exact font widths, letter spacing and icon rasterization.
- Spacing under 4px, a 1-2px header difference, and the Scan footer clip.

## Next Steps (Ordered)
1. [ ] Bind Optics to the frame edit.
2. [ ] Bind Metering and Roll Analysis to the frame edit.
3. [ ] Bind the Roll Setup controls to the frame edit.
4. [ ] Bind the Frame tab controls to the frame edit.

## Completed (Recent)
- `HEAD` desktop: bind Raw Decode to the edit.
- `8a45f489` desktop: load and save the frame edit.
- `2763e8d8` desktop: own the frame edit document.
- `cbd6bdbf` desktop: close the ten-commit batch.
- `5de4ba4f` desktop: wire the average toggles.
- `1a78a6c5` desktop: build the Metering card.
- `76487cb6` desktop: build the Optics card.
- `6a84944a` desktop: build the Raw Decode card.
- `7a8166a2` desktop: share the bundled fixture.
- `6268316a` desktop: read the Roll Analysis baseline.
- `ac6877fd` desktop: build the Light Table view.
- `bbea3e1c` desktop: give Scan its source icons.

## Verification
- Targeted Raw Decode tests passed: 4 tests.
- `cargo check --workspace` passes. `cargo fmt --all --check` passes.
- Clippy is blocked by existing lints in `negpy-backend` and older desktop
  modules (`manual_is_multiple_of`, `too_many_arguments`). None are new.
- Roll capture: total 0.700, layout 0.731. The card reads the stored roll
  name; the copied golden profile shows w9, the live profile shows vv9.
- Metering and the three new cards render in the Roll stack.
- Light Table capture: total 0.708, layout 0.685. Tile metrics stay deferred.

## Constraints / Gotchas
- Never edit `negpy/`; `negpy_core` stays its own workspace.
- Reference captures: `negpy_egui/tests/gui/golden/tall/`.
- Run the app with `cd negpy_egui && cargo run --release --bin negfe`.
- Capture with `NEGPY_GUI_PROFILE=/home/bruno/Dokumenty/NegPy` and
  `NEGPY_GUI_FOLDER=/home/bruno/WORK/svetovy_vylet/skeny/vv9`.
- A capture can rasterize mid-scroll and under-score the rail.
- Generated captures stay untracked under `tests/gui/actual/`.
- `negfe` is the manual target; `negfe-capture` is the fixed matrix.
