# Development Ledger
**Last Updated:** 2026-11-19 18:10 | **Last Commit:** `pending` desktop: share the bundled fixture

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up. E5 shell work is active.

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
1. [ ] E5: give the Raw Decode card its controls.
2. [ ] E5: give the Optics card its controls.
3. [ ] E5: give the Metering card its controls.
4. [ ] E5: wire the Use Average toggles.

## Completed (Recent)
- `pending` desktop: share the bundled fixture.
- `6268316a` desktop: read the Roll Analysis baseline.
- `ac6877fd` desktop: build the Light Table view.
- `bbea3e1c` desktop: give Scan its source icons.

## Verification
- `cargo test -p negpy-desktop` passes: 119 library tests. Fixture tests
  cover the bundled reference file.
- `cargo check --workspace` passes. `cargo fmt --all --check` passes.
- Clippy is blocked by existing lints in `negpy-backend` and older desktop
  modules (`manual_is_multiple_of`, `too_many_arguments`). None are new.
- Roll capture: total 0.700, layout 0.731. The card reads the stored roll
  name; the copied golden profile shows w9, the live profile shows vv9.
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
