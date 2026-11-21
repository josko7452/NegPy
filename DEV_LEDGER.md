# Development Ledger
**Last Updated:** 2026-11-22 08:05 | **Last Commit:** `45c30176 desktop: drop the placeholder cards`

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`. The fifth batch closed every C1 gap:
Favorites, the sheet picker, scene detection, metadata and export presets,
hand-picked bounds and the placeholder cards. Next is the C2 list in
`MIGRATION_PLAN.md` §9.11, starting with the pick view.

## Batch Protocol
After commit #10, run the source audit in `MIGRATION_PLAN.md` §8.7. Write its
findings into §9.11 and this ledger, then run `cargo check --workspace` and
`cargo test --workspace`. The audit does not authorize an 11th commit.

## Next Steps (Ordered)
1. [ ] Bounds pick: sample the uncropped pick view, not the cropped frame
   (§9.11 C2-1).
2. [ ] Detect Scenes: move the strip decode off the UI thread (§9.11 C2-2),
   then analyze an accepted scene (§9.11 C2-3).
3. [ ] Metadata Presets: honour a multi-frame selection and card locks
   (§9.11 C2-4).
4. [ ] Contact Sheet: seed left-out from `excluded` marks and make a tile
   clickable (§9.11 C2-5).
5. [ ] Export to Preset: honour the preset `overwrite` flag (§9.11 C2-6).
6. [ ] Favorites: reorder the chosen sliders (§9.11 C2-7).
7. [ ] Pick Dmin and Dmax on the canvas: the uncropped view (C2-1).
8. [ ] Camera capture: blocked until a human picks the hardware target.
9. [ ] GPU render path: a multi-batch epic (§9.10 B1-9).

## Completed (Recent)
- `desktop`: History shows its two real cards; the duplicate Metering card and
  the placeholder painter are gone.
- `favourites`: the Favorites page lists seven Exposure sliders, saves the
  chosen ids under `favourite_sliders` and mirrors their values.
- `roll-analysis`: Detect measures each strip frame's neutral axis, groups the
  frames under another light and Accept writes each group as a scene.
- `contact-sheet`: Pick Frames lists the strip; a left-out frame leaves the
  entries, the page plan and the export.
- `export`: Export to Preset runs the selected preset on the non-rejected
  strip, with the preset's encoder fields and destination.
- `metadata`: the Presets page takes an Apply to Frame or Roll choice.
- `desktop`: a set Dmin or Dmax has a clear button, and the canvas paints each
  picked point with its label.
- `desktop`: a Dmin or Dmax click samples the unmixed log patch, stores the
  manual bound and its point, clears the stored bounds and disarms.
- `backend`: `sample_log_patch` reads the unmixed log density under a point.
- Earlier batches: crosstalk, preset, culling, crop-guide and export forms,
  analysis scopes, local masks, crop confirm, Undo/Redo, zoom, Tools, Print,
  Roll Average, Crop All, Find and the recipe line.

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
- `MIGRATION_PLAN.md` section 9 is the gap register; §9.11 is the fifth audit.
- `Draw` starts the crop tool; `has_crop` holds the stored rect. Its interaction
  must come from `Draw`, never from a stored rect, or Enter cannot confirm.
- The local mask tool returns before it claims a drag when its mode is `Off`.
- Bounds picks sample a geometry-processed proxy; see C2-1 before trusting a
  pick on a cropped frame.
- Detect decodes the strip in `update`; see C2-2 before a long roll.
- Retouch bakes are wired ahead of both engines; the scopes are binned from the
  display and normalized frames.
- No `wgpu` backend or WGSL shader exists in `negpy_egui/`.
- Camera capture has no hardware adapter in the Rust backend.
- The Scan card's Prescan and Scan as Roll are disabled: no workers.
- The mask picker edits the selected mask; an oval selects inside its outline.
- `receive_render` accepts output by generation and request hash.
- Generated captures stay untracked under `tests/gui/actual/`.
- Session batches are 10 commits; this ledger stays under 120 lines.
- `negpy_egui/AGENTS.md` still says 5 commits and 60 ledger lines. A human must
  reconcile it; the plan and this ledger carry the 10/120 override.
