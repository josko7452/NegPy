# Development Ledger
**Last Updated:** 2026-11-21 21:30 | **Last Commit:** `pending prefix cache`

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`. The sixth batch started the GPU epic:
the device, resources, five shaders and a prefix chain with parity tests. Next
is the GPU audit's G-2 and G-3 in `MIGRATION_PLAN.md` §9.12, then its G-1.

## Batch Protocol
After commit #10, run the source audit in `MIGRATION_PLAN.md` §8.7. Write its
findings into §9.11 and this ledger, then run `cargo check --workspace` and
`cargo test --workspace`. The audit does not authorize an 11th commit.

## Next Steps (Ordered)
1. [ ] GPU: add stage caching and a texture pool, then the tiled path
   (§9.12 G-4).
2. [ ] GPU: pin shader output bits and add per-channel and camera cases
   (§9.12 G-5, G-6).
3. [ ] Bounds pick: sample the uncropped pick view, not the cropped frame
   (§9.11 C2-1).
4. [ ] Detect Scenes: move the strip decode off the UI thread (§9.11 C2-2),
   then analyze an accepted scene (§9.11 C2-3).
5. [ ] Metadata Presets: honour a multi-frame selection and card locks
   (§9.11 C2-4).
6. [ ] Contact Sheet: seed left-out from `excluded` marks and make a tile
   clickable (§9.11 C2-5).
7. [ ] Export to Preset: honour the preset `overwrite` flag (§9.11 C2-6).
8. [ ] Favorites: reorder the chosen sliders (§9.11 C2-7).
9. [ ] Camera capture: blocked until a human picks the hardware target.

## Completed (Recent)
- `backend`: the GPU engine keeps the geometry and normalization textures until
  the source or prefix key changes.
- `backend`: the GPU engine has a size-keyed texture pool and returns the print
  chain's textures after each frame.
- `backend`: GPU eligibility is pinned to the frames the current chain covers,
  and a config with a local mask or carrier stays on the CPU.
- `desktop`: the render worker uses the GPU chain for a print frame that
  needs no local map, print LUT or carrier, and keeps the CPU engine otherwise.
- `backend`: the GPU vignette pass matches the core finish kernel; carrier
  frames stay on the CPU.
- `backend`: the GPU print pass runs the exposure curve and its parity test
  matches `render_negative` for color and B&W frames.
- `backend`: a GPU crop pass copies the resolved window and the chain exposes
  a cropped base run.
- `backend`: the GPU device opens once, textures and shaders are cached, and
  the geometry, normalization, encode, histogram and auto-crop passes have
  parity tests.
- `backend`: the GPU chain runs the prefix over one texture chain and reads
  `use_gpu` before it opens a device.
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
  `cargo fmt --all --check` pass after the ten commits. The GPU parity tests
  ran on the local Vulkan adapter and skip without one.
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
- `MIGRATION_PLAN.md` section 9 is the gap register; §9.12 is the sixth audit.
- `Draw` starts the crop tool; `has_crop` holds the stored rect. Its interaction
  must come from `Draw`, never from a stored rect, or Enter cannot confirm.
- The local mask tool returns before it claims a drag when its mode is `Off`.
- Bounds picks sample a geometry-processed proxy; see C2-1 before trusting a
  pick on a cropped frame.
- Detect decodes the strip in `update`; see C2-2 before a long roll.
- Retouch bakes are wired ahead of both engines; the scopes are binned from the
  display and normalized frames.
- `negpy_egui` holds five ported WGSL shaders. The other 28 upstream shaders
  have no Rust pass, and the desktop worker still renders on the CPU.
- The GPU chain reads CPU-measured bounds. The meters themselves are not on the
  GPU, so a GPU-only run would need the measurement pass first.
- `GpuTexture::readback` waits on the queue; no pipelining or stage cache exists.
- Camera capture has no hardware adapter in the Rust backend.
- The Scan card's Prescan and Scan as Roll are disabled: no workers.
- The mask picker edits the selected mask; an oval selects inside its outline.
- `receive_render` accepts output by generation and request hash.
- Generated captures stay untracked under `tests/gui/actual/`.
- Session batches are 10 commits; this ledger stays under 120 lines.
- `negpy_egui/AGENTS.md` still says 5 commits and 60 ledger lines. A human must
  reconcile it; the plan and this ledger carry the 10/120 override.
