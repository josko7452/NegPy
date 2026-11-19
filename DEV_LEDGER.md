# Development Ledger
**Last Updated:** 2026-11-20 01:05 | **Last Commit:** `HEAD` desktop: select frames from the strip

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
1. [ ] Re-render the canvas on edit changes through one coalesced request.
2. [ ] Feed real render metrics into Analysis and its readouts.
3. [ ] Re-render the canvas on edit changes through one coalesced request.
4. [ ] Feed real render metrics into Analysis and its readouts.
5. [ ] Keep every section's open state in page state; test each header toggle.
6. [ ] Wire Export Roll and the Export footer to the export service.
7. [ ] Apply soft proof to the preview and expose Favorites/Presets.
8. [ ] Wire Scan and Contact Sheet actions; propagate Library open/status.
9. [ ] Wire the P3 card actions, then add dialogs and shortcuts.

## Completed (Recent)
- `HEAD` desktop: select frames from the strip.
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
