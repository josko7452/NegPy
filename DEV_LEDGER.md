# Development Ledger
**Last Updated:** 2026-11-22 05:50 | **Last Commit:** `pending export: run a saved preset on the strip`

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`. The fourth batch closed the crosstalk,
preset, culling, crop-guide and export-form gaps. Next add the missing GPU
engine and finish the dead card actions in `MIGRATION_PLAN.md` §9.11.

## Batch Protocol
After commit #10, run the source audit in `MIGRATION_PLAN.md` §8.7. Write its
findings into §9.11 and this ledger, then run `cargo check --workspace` and
`cargo test --workspace`. The audit does not authorize an 11th commit.

## Next Steps (Ordered)
1. [ ] Add the GPU render path: no `wgpu` backend or WGSL shader is present in
   `negpy_egui/`, so every GPU-only stage is missing (`MIGRATION_PLAN.md` D10,
   §9.10 B1-9). This is a multi-batch epic, not one commit.
2. [ ] Camera capture is blocked: the Rust backend has only the `Camera` and
   `LightSource` traits, with no gphoto2, Scanlight or simulated desktop
   adapter and no capture worker. A human must choose the hardware target.
3. [ ] Pick Dmin and Dmax by hand on the canvas (store, clear and marker landed; the Python uncropped pick view is not ported).
4. [ ] Metadata Presets: apply to the open frame or the whole roll (landed; a multi-frame selection is still not modelled).
5. [ ] Export presets: run a preset on the strip (landed; the Presets page runs one selected preset).
6. [ ] Contact Sheet: Pick Frames leaves a frame out or puts it back.
7. [ ] Roll Analysis: Detect groups the strip into scenes for Accept or Discard.
8. [ ] Frame Favorites: the stored slider list and its editor.

## Completed (Recent)
- `desktop`: Dmin and Dmax pick on the canvas samples the unmixed log patch,
  stores the manual bound and its point, clears the stored bounds and disarms.
- `desktop`: a set Dmin or Dmax has a clear × in the Pick by Hand fold, and the
  canvas paints each picked point with its label.
- `metadata`: the Presets page takes an Apply to Frame or Roll choice and writes
  the stored fields over every other frame of the roll.
- `export`: Export to Preset runs the selected preset on every non-rejected
  frame, with the preset's encoder fields and destination.
- `calibration`: the Calibration card lists the crosstalk profiles, bakes the
  selected matrix and process into the edit and clears the stored bounds.
- `metadata`: the Presets page lists the store's presets and saves, applies and
  deletes them through the settings catalog.
- `export`: the Presets page selects, renames, enables and deletes a preset.
- `export`: the File page shows and stores the bit depth, TIFF, PNG, JXL and
  WebP encoder fields and the Print size and DPI.
- `export`: the absolute destination branch shows and stores the export path.
- `contact-sheet`: the dialog reads its page count from the sheet layout and
  stops at the last page.
- `light-table`: Keep, Reject, Auto Crop and Export respond; Keep and Reject mark
  the open frame.
- `light-table`: Keep and Reject write the frame marks the Library search reads,
  the grid paints them, and Export skips a rejected frame.
- `crop`: the guide choice persists, the crop box paints its composition guide,
  and the Ratio crosshair measures the film frame's closest standard ratio.
- `2ff82255` desktop: Crop All keeps a frame edited during the run.
- `1d7fe335` desktop: Find keeps its list and opens the active row.
- `34fec0dc` desktop: the recipe follows the frame's paper, density and grade.
- `e40416f6` desktop: a shortcut registry dispatches shared toolbar actions.
- `e747a22c` desktop: Ctrl+K opens Find over the shell's cards and actions.
- `50850d3c` desktop: Print opens the Export form.
- `8d463b9f` desktop: Tools arms a canvas tool and opens its page.
- `805c002b` desktop: Roll Average applies or clears Luma, Color and Cast.
- `9957d874` desktop: presets, the wheel and double-click zoom the canvas.
- `694e16b4` desktop: Crop All runs a strip worker and persists each crop.
- Earlier batches: the analysis scopes, local mask editing, crop confirm,
  Photo/Roll, rotate/flip and Undo/Redo landed before `694e16b4`.
- The render worker returns the request and painted hashes; Auto Crop freezes
  before the texture is shown.
- Preview and export run the retouch bakes; Export writes a sidecar when its
  toggle is on.

## Verification
- This batch: `cargo test --workspace`, `cargo check --workspace` and
  `cargo fmt --all --check` pass after the ten commits.
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
- Retouch bakes are wired ahead of both engines; the scopes are binned from the
  display and normalized frames.
- No `wgpu` backend or WGSL shader exists in `negpy_egui/`. The shell draws
  textures through egui only.
- Camera capture has no hardware adapter in the Rust backend. The Scan card's
  camera controls stay informational.
- The Scan card's Prescan and Scan as Roll are disabled: their workers are not
  ported.
- Photo/Roll, rotate/flip, Undo/Redo and Square to Film reach the edit. Zoom,
  Tools, Print, Roll Average, Crop All Frames, Find and the recipe line now
  respond (§9.10).
- The post-batch source audit is §9.11 in `MIGRATION_PLAN.md`.
- The mask picker edits the selected mask; ovals select inside their outline and
  cards near their axis.
- `receive_render` accepts output by generation and request hash, not by the
  config hash that painted it.
- The Light Table capture names one frame; the golden shows the whole roll.
- Generated captures stay untracked under `tests/gui/actual/`.
- `negfe` is the manual target; `negfe-capture` is the fixed matrix.
- Session batches are 10 commits; the ledger limit is 120 lines
  (`MIGRATION_PLAN.md` §8.2–§8.4).
- `negpy_egui/AGENTS.md` still says 5 commits and 60 ledger lines. A human must
  reconcile it; the plan and this ledger carry the 10/120 override.
