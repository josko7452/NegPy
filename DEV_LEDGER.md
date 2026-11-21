# Development Ledger
**Last Updated:** 2026-11-22 00:20 | **Last Commit:** `8e1e2d23 lab: measure auto masking on the GPU`

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`. The eighth batch closed the CLAHE
blocker and moved CLAHE, Lab and toning onto the GPU tail, with the crop fused
into toning. The finish stage and the alt-process passes stay on the CPU.

## Batch Protocol
After commit #10, run the source audit in `MIGRATION_PLAN.md` §8.7. Write its
findings into §9.11 and this ledger, then run `cargo check --workspace` and
`cargo test --workspace`. The audit does not authorize an 11th commit.

## Next Steps (Ordered)
1. [ ] GPU: port the finish and carrier passes so the chain replaces the
   whole tail (§9.12 G-1).
2. [ ] GPU: port the alt-process and RL sharpen passes, then drop the
   alt-process eligibility gate.
3. [ ] GPU: add the tiled export path (§9.12 G-4).
4. [ ] Bounds pick: sample the uncropped pick view, not the cropped frame
   (§9.11 C2-1).
6. [ ] Detect Scenes: move the strip decode off the UI thread (§9.11 C2-2),
   then analyze an accepted scene (§9.11 C2-3).
7. [ ] Metadata Presets: honour a multi-frame selection and card locks
   (§9.11 C2-4).
8. [ ] Contact Sheet: seed left-out from `excluded` marks and make a tile
   clickable (§9.11 C2-5).
9. [ ] Export to Preset: honour the preset `overwrite` flag (§9.11 C2-6).
10. [ ] Favorites: reorder the chosen sliders (§9.11 C2-7).
11. [ ] Camera capture: blocked until a human picks the hardware target.

## Completed (Recent)
- `backend`: the CLAHE passes run on the GPU. The empty-tile CDF divides by one,
  so a dimension that is not a multiple of 8 no longer blends NaN.
- `backend`: the Lab pass and its sharpen and chroma low-pass preconditions run
  on the GPU.
- `backend`: the toning pass runs on the GPU and performs the crop, as upstream.
- `backend`: the GPU tail now runs print, CLAHE, Lab and toning, and a cropped
  frame matches the CPU engine.
- `backend`: a cropped GPU frame was cropped twice; the chain now crops once.
- `backend`: the GPU worker meters through the crop window, matching the CPU.
- `backend`: auto masking measures its grain on the device in two passes.
- `backend`: an alt-process frame stays on the CPU engine.
- `backend`: the GPU print, per-channel print and vignette passes have pinned
  output hashes, and a transfer-path camera matrix matches the core.
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
- Earlier batches: History cards, Favorites, Detect Scenes, Contact Sheet,
  Export to Preset, Metadata Presets, Dmin/Dmax picks, crosstalk, culling,
  analysis scopes, local masks, crop confirm, Undo/Redo and zoom.

## Verification
- `cargo check --workspace`, `cargo test --workspace` and
  `cargo fmt --all --check` pass. `timing::tests` is a wall-clock test that
  flakes under load and passes alone. The GPU parity tests ran on the local
  Vulkan adapter and skip without one.
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
- `MIGRATION_PLAN.md` section 9 is the gap register; §9.12 holds the CLAHE
  resolution. The empty tile row is why the CDF divides by `max(total, 1)`.
- The Lab pass decodes its encoded input and writes scene-linear pixels.
  Toning reads linear at `crop_offset`; finish reads the cropped frame.
- `Draw` starts the crop tool; `has_crop` holds the stored rect. Its interaction
  must come from `Draw`, never from a stored rect, or Enter cannot confirm.
- The local mask tool returns before it claims a drag when its mode is `Off`.
- Bounds picks sample a geometry-processed proxy; see C2-1 before trusting a
  pick on a cropped frame.
- Detect decodes the strip in `update`; see C2-2 before a long roll.
- Retouch bakes are wired ahead of both engines; the scopes are binned from the
  display and normalized frames.
- `negpy_egui` holds eight ported WGSL shaders. The other 25 upstream shaders
  have no Rust pass. The worker uses the GPU only for an eligible print frame.
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
