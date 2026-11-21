# Development Ledger
**Last Updated:** 2026-11-21 14:42 | **Last Commit:** `620593c1` export: write the sidecar the toggle promises

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`. The crop, retouch and mask paths now
reach the CPU render. Next add the missing GPU engine and finish the dead
toolbar and card actions from the section 9 audit.

## Next Steps (Ordered)
1. [ ] Add the GPU render path: no `wgpu` backend or WGSL shader is present in
   `negpy_egui/`, so every GPU-only stage is missing (`MIGRATION_PLAN.md` D10).
2. [ ] Add the tone-limited local masks: the print curve accepts a keyed alpha
   plane and key parameters, but the engine passes neither.
3. [ ] Wire the dead toolbar and card actions from the section 9 audit
   (Photo/Roll mode, zoom, rotate/flip, Tools, Undo/Redo, Print, Roll Average,
   Prescan, Scan as Roll).

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
- The canvas emits typed Heal, Line and Clone gestures. Heal and Scratch share
  the heal-stroke path; Clone takes an Alt-click source.
- The local stage now reaches the print for stops, grade and flash. A canvas
  mask selects and moves its vertices; the Dodge & Burn card edits the selected
  mask.
- The Analysis chart paints the render histogram and the Log toggle is live.
- Export writes a `.negpy` sidecar when its toggle is on. Input and Output ICC
  profiles load from the profile's ICC folder.
- Folder as Roll imports the Scan card's output folder. Prescan and Scan as Roll
  are disabled until their worker is ported.
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
- Retouch bakes are wired ahead of both engines. Unlimited local masks reach
  the print; tone-limited keyed masks do not.
- The histogram is binned from the display-encoded frame on the card strip. The
  Analysis card still reads only the curve and readouts.
- Retouch runs on the source proxy and the full-resolution export. Its IR plane
  is oriented and resized with the source.
- No `wgpu` backend or WGSL shader exists in `negpy_egui/`. The shell draws
  textures through egui only.
- The Scan card's Prescan and Scan as Roll are disabled: their workers are not
  ported.
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
