# Development Ledger
**Last Updated:** 2026-11-20 21:10 | **Last Commit:** `3becfc36` local: drag a card gradient mask

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`. Close the shell-session gaps before
adding panels or dialogs; the next ten commits are authorized.

## UX Target (supersedes pixel parity)
The goal is the same workflow, controls, order, grouping and density as NegPy.
Layout score is a diagnostic. Stop when the control is usable and in place.

## Deferred (not planned)
- Film Strip cells, canvas bar segments and Contact Sheet tile metrics.
- Exact font widths, letter spacing and icon rasterization.
- Spacing under 4px, a 1-2px header difference, and the Scan footer clip.

## Next Steps (Ordered)
1. [ ] Add canvas strokes for Heal, Scratch, Line and Clone.
2. [ ] Add local mask selection and vertex editing on the canvas.

## Completed (Recent)
- `3becfc36` local: drag a card gradient mask.
- `fdbb647e` local: drag an oval mask on the canvas.
- `fa10239c` local: draw a mask on the canvas.
- `161bea50` roll: apply the saved defaults when a roll opens.
- `e3e81fe4` roll: save the open frame as the baseline.
- `3b621f2e` metering: store the measured bounds on the frame.
- `74c7fd08` frame: reshape a drawn crop to the chosen ratio.
- `2e096eb0` frame: apply the Auto Crop detection.
- `ab7e9d68` frame: paint the crop box and cursor.
- `309c0ea2` frame: keep the region channel and retouch tool.
- `799aaf76` metadata: clear cards and infer the folder date.
- `ce98e88a` export: save the current form as a preset.
- `2aceb225` metering: reset trims and measure a frame alone.
- `32b82476` history: restore steps and save work prints.
- `00401b1c` desktop: apply soft proof to the preview.
- `cb46520f` desktop: export the roll on a worker.
- `e9f27f85` desktop: keep card folds in page state.
- `c64741b4` desktop: feed render metrics into Analysis.

## Verification
- `cargo test --workspace`, `cargo check --workspace` and
  `cargo fmt --all --check` pass.
- Clippy is blocked by existing lints in `negpy-backend` and older desktop
  modules (`manual_is_multiple_of`, `too_many_arguments`). None are new.
- The golden vv9 layout baseline is below target on Light Table and Contact Sheet.

## Constraints / Gotchas
- Never edit `negpy/`; `negpy_core` stays its own workspace.
- Reference captures: `negpy_egui/tests/gui/golden/tall/`.
- Run the app with `cd negpy_egui && cargo run --release --bin negfe`.
- Manual profile `/home/bruno/Dokumenty/NegPy`; roll `vv9`.
- Without `--profile`, Import cannot write and edits save nowhere.
- A section rebuilt in `show` loses its open state on the next frame.
- `MIGRATION_PLAN.md` section 9 is the gap register and evidence index.
- The Light Table capture names one frame; the golden shows the whole roll.
- Generated captures stay untracked under `tests/gui/actual/`.
- `negfe` is the manual target; `negfe-capture` is the fixed matrix.
