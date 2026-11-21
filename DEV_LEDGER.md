# Development Ledger
**Last Updated:** 2026-11-22 00:45 | **Last Commit:** `1d7fe335` desktop: keep the Find list keyboard-navigable

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`. The crop, retouch and mask paths now
reach the CPU render. Next add the missing GPU engine and finish the dead
toolbar and card actions in `MIGRATION_PLAN.md` §9.10.

## Batch Protocol
After commit #10, run the source audit in `MIGRATION_PLAN.md` §8.7. Write its
findings into §9.10 and this ledger, then run `cargo check --workspace` and
`cargo test --workspace`. The audit does not authorize an 11th commit.

## Next Steps (Ordered)
1. [ ] Add the GPU render path: no `wgpu` backend or WGSL shader is present in
   `negpy_egui/`, so every GPU-only stage is missing (`MIGRATION_PLAN.md` D10,
   §9.10 B1-9). This is a multi-batch epic, not one commit.
2. [ ] Add the GPU render path (`MIGRATION_PLAN.md` D10, §9.10 B1-9). This is
   a multi-batch epic, not one commit.

## Completed (Recent)
- Crop All Frames now runs a strip worker over `batch_autocrop`, applies each
  resolved crop and reports saved/kept/failed.
- Zoom presets, wheel zoom and double-click fit/original reach the canvas; the
  readout prints the true pixel scale.
- Roll Average applies or clears Luma, Color and Cast on the open frame.
- Tools arms Crop, Draw Region, a local mask or a retouch tool and opens the
  page that owns it.
- Print opens the Export tab on its file page.
- Find takes Ctrl+K, filters the shell's cards and actions, and opens a
  result on its tab.
- A shortcut registry dispatches navigation, rotate/flip, undo/redo, zoom,
  Print, Roll Average, Crop All and Tools.
- The top-bar recipe follows the frame's paper, Print Density and grade, with
  the auto markers.
- Find keeps its list open, moves with Up/Down and opens the active row on
  Enter; Escape closes it.
- Crop All snapshots a detection fingerprint and keeps a frame edited during
  the run.
- `5b5b11c9` plan: §9.10 closes eight rows and lists nine open ones.
- `0b08c9a1` rail: the curve chart marks a channel that clips at the black or
  white edge bin.
- `ef09fe41` finish: the dead `Drop Empty Corners` button is retired; Crop
  Empty Corners on Geometry is its control.
- `a384cfb7` toolbar: the rotate and flip buttons turn the open frame, and a
  flipped frame turns the pipeline the opposite way, as Python does.
- `a09e3fa1` toolbar: Undo and Redo step a per-frame config stack; Square to Film
  calls `squared_geometry` on the open proxy and redraws.
- `24c00efa` toolbar: Photo and Roll select the Frame and Roll tabs.
- `a36ecb9f` ledger: records the local and analysis commits.
- `4f2a8bdf` rail: the worker bins the waveform, parade and vectorscope; the
  Analysis card switches between curve, waveform, parade and vectorscope, and
  the curve also paints the normalized density histogram.
- `a9836f82` local: an oval and a card mask are hit-tested by their outline and
  axis, not by their control points.
- `62a3c939` local: the tone-limited mask key, zone and softness reach the print
  curve; the key edges invert the achromatic curve.
- `c5a42cf8` local: the Dodge & Burn card sets the selected mask's `MaskKey`.
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
- Export writes a `.negpy` sidecar when its toggle is on. Input and Output ICC
  profiles load from the profile's ICC folder.
- Folder as Roll imports the Scan card's output folder. Prescan and Scan as Roll
  are disabled until their worker is ported.
- `0f62b2e9` library: keep the card open state after a reload.
- `09153197` rail: keep the Analysis card collapsed.
- `35d7c9ae` frame: confirm the crop with Enter or right-click.
- `383a26de` frame: draw the crop box while dragging.
- `6ab4469a` frame: keep a typed crop drag.
- `3becfc36` local: drag a card gradient mask.
- `fdbb647e` local: drag an oval mask on the canvas.
- `fa10239c` local: draw a mask on the canvas.

## Verification
- This batch: `cargo test --workspace`, `cargo check --workspace` and
  `cargo fmt --all --check` passed after the ten commits.
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
- Retouch bakes are wired ahead of both engines. Unlimited and tone-limited
  local masks reach the print; the card sets the key and the zone rail.
- The scopes and density histogram are binned from the display and normalized
  frames; the Analysis card paints each view.
- Retouch runs on the source proxy and the full-resolution export. Its IR plane
  is oriented and resized with the source.
- No `wgpu` backend or WGSL shader exists in `negpy_egui/`. The shell draws
  textures through egui only.
- The Scan card's Prescan and Scan as Roll are disabled: their workers are not
  ported.
- Photo/Roll, rotate/flip, Undo/Redo and Square to Film reach the edit. Zoom,
  Tools, Print, Roll Average and Crop All Frames are still painted (§9.10).
- The post-batch source audit is §9.10 in `MIGRATION_PLAN.md`.
- The mask picker edits the selected mask; ovals select inside their outline and
  cards near their axis.
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
