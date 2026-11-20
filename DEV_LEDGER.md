# Development Ledger
**Last Updated:** 2026-11-20 12:10 | **Last Commit:** `HEAD` desktop: store typed metadata fields

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
1. [ ] Wire the Library import status and open request.
2. [ ] Give Optics real flat-field profile actions.
3. [ ] Drive the Scan controls from the scanner service.
4. [ ] Wire the remaining P3 card actions.

## Completed (Recent)
- `HEAD` desktop: store typed metadata fields.
- `cea3ec8e` desktop: map paper and transform controls.
- `041f7b0d` desktop: map filtration and tone channels.
- `9e0eac36` desktop: map export ICC fields to the edit.
- `1f931b2e` desktop: map geometry fields to their config.
- `7f91674a` desktop: map exposure fields to their config.
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
