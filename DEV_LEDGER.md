# Development Ledger
**Last Updated:** 2026-11-21 12:10 | **Last Commit:** `bece5c3c` desktop: keep every card state under the tab collapse

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`. The crop canvas accepts Enter and keeps
its rectangle, but a confirmed crop and an Auto Crop do not yet prove the canvas
redrew. Make that render path observable before the remaining tool work.

## Next Steps (Ordered)
1. [ ] Add canvas strokes for Heal, Scratch, Line and Clone
   (`MIGRATION_PLAN.md` §9.4 P3-11).
2. [ ] Add local mask selection and vertex editing on the canvas.
3. [ ] Feed the Analysis chart its histogram and readouts; the card strip has
   the buffer, the chart itself still reads only the curve.
4. [ ] Wire the remaining Export controls: input ICC and sidecar scope
   (`MIGRATION_PLAN.md` §9.5 P3b-14, P3b-15).

## Completed (Recent)
- `98d7b11e` frame: keep crop drawing active until confirm.
- The render worker returns the request hash and the painted config hash; the
  shell accepts a render only when both generation and request hash match.
- Auto Crop freezes before the texture is shown; a second request finds the
  painted hash and submits nothing.
- The render worker returns the `(4, 256)` output histogram of the texture it
  painted; the Filtration and Tone card headers paint it.
- `TabSummaryAction::Collapse` now toggles a page flag; each page keeps its own
  card open states across the folded view.
- Preview and export both run the retouch source bakes. The source hash carries
  the IR, heal, dust, hair and clone tokens.
- `53efd5c5` scan: keep the Camera card state across modes.
- `0f62b2e9` library: keep the card open state after a reload.
- `09153197` rail: keep the Analysis card collapsed.
- `35d7c9ae` frame: confirm the crop with Enter or right-click.
- `383a26de` frame: draw the crop box while dragging.
- `6ab4469a` frame: keep a typed crop drag.
- `3becfc36` local: drag a card gradient mask.
- `fdbb647e` local: drag an oval mask on the canvas.
- `fa10239c` local: draw a mask on the canvas.

## Verification
- The last full run before `98d7b11e`: `cargo test --workspace`,
  `cargo check --workspace` and `cargo fmt --all --check` passed.
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
- UX target: the same workflow, controls, order and density as NegPy; layout
  score is a diagnostic.
- Deferred: Film Strip cells, canvas bar segments, Contact Sheet tile metrics,
  exact font widths and sub-4px spacing.
- `MIGRATION_PLAN.md` section 9 is the gap register and evidence index.
- `MIGRATION_PLAN.md` section 10 is the crop-interaction plan.
- `Draw` starts the crop tool; `has_crop` holds the stored rect. Its interaction
  must come from `Draw`, never from a stored rect, or Enter cannot confirm.
- The local mask tool returns before it claims a drag when its mode is `Off`.
- Retouch bakes are wired ahead of both engines. The local mask stage is still
  absent from the Rust exposure pass, so canvas masks remain unwired.
- The histogram is binned from the display-encoded frame on the card strip. The
  Analysis card still reads only the curve and readouts.
- Retouch runs on the source proxy and the full-resolution export. Its IR plane
  is oriented and resized with the source.
- `receive_render` accepts output by generation and request hash, not by the
  config hash that painted it.
- The render worker resolves Auto Crop in its own config, then returns a hash
  for that resolved config. The shell freezes the rect after it shows the frame.
- The Light Table capture names one frame; the golden shows the whole roll.
- Generated captures stay untracked under `tests/gui/actual/`.
- `negfe` is the manual target; `negfe-capture` is the fixed matrix.
- Session batches are 10 commits; the ledger limit is 120 lines
  (`MIGRATION_PLAN.md` §8.2–§8.4).
- `negpy_egui/AGENTS.md` still says 5 commits and 60 ledger lines. A human must
  reconcile it; the plan and this ledger carry the 10/120 override.
