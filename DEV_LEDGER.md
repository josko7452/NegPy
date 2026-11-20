# Development Ledger
**Last Updated:** 2026-11-20 10:30 | **Last Commit:** `pending` desktop: map export ICC fields to the edit

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`. Wiring audit is complete; close the
shell-session gaps before adding panels or dialogs.

## UX Target (supersedes pixel parity)
The goal is the same workflow, controls, order, grouping and density as NegPy.
Layout score is a diagnostic. Stop when the control is usable and in place.

## Deferred (not planned)
- Film Strip cells, canvas bar segments and Contact Sheet tile metrics.
- Exact font widths, letter spacing and icon rasterization.
- Spacing under 4px, a 1-2px header difference, and the Scan footer clip.

## Next Steps (Ordered)
1. [ ] Wire Scan and Contact Sheet actions; propagate Library open/status.
2. [ ] Wire the P3 card actions, then add dialogs and shortcuts.
3. [ ] Map the remaining P3b fields: region, channel, damping.

## Completed (Recent)
- `pending` desktop: map export ICC fields to the edit.
- `1f931b2e` desktop: map geometry fields to their config.
- `7f91674a` desktop: map exposure fields to their config.
- `00401b1c` desktop: apply soft proof to the preview.
- `cb46520f` desktop: export the roll on a worker.
- `e9f27f85` desktop: keep card folds in page state.
- `c64741b4` desktop: feed render metrics into Analysis.
- `2cc2b7c9` desktop: render edits on a worker.
- `4a39af15` desktop: select frames from the strip.
- `66a42371` desktop: paint the empty workspace.
- `1dacaff8` desktop: open the Library roll.
- `2f578cec` desktop: resolve the launch profile.
- `7b6f6e14` desktop: justify the Light Table rows.
- `96c75264` desktop: refresh the Library rolls.
- `d1bec40b` desktop: sort the Library rows.
- `89192452` desktop: wire the Library Import menu.
- `5bfe2236` backend: store the proof condition in session settings.
- `c2bb6ed8` backend: keep the proof condition out of the export edit.
- `9be68fcc` desktop: load the saved export and metadata presets.
- `7d7efc33` desktop: bind the Export proof and sidecars.
- `99a8313a` desktop: bind the Export file page to the edit.

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
