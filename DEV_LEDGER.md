# Development Ledger
**Last Updated:** 2026-11-22 20:21 | **Last Commit:** `0361fc73 detect: measure the strip off the UI thread`

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`. The tenth batch closed frame
selection, toolbar collapse, tab overflow, the minimum window, GPU preference
loading, histogram data and drawing, contact sheet triage and Detect Scenes.
The plan is the active register; completed history is in
`MIGRATION_PLAN_ARCHIVE.md`.

## Batch Protocol
After commit #10, run the source audit in `MIGRATION_PLAN.md`. Write its
findings here, then run `cargo check --workspace` and
`cargo test --workspace`. The audit does not authorize an 11th commit.

## Next Steps (Ordered)
1. [ ] Crop handles: move a corner or the whole pending rectangle (C1-1).
2. [ ] Masks: bind local-map, key and contrast-mask textures, then prove a
   masked GPU frame against the CPU (G1-1).
3. [ ] Export: decide the GPU path and report the engine (G1-4, G1-6).
4. [ ] Evidence: regenerate both golden sets and close L1-6 and H1-6.
5. [ ] Startup: render the first launched frame through the worker (G1-3).
6. [ ] Metadata Presets: honour a multi-frame selection and card locks.
7. [ ] Favorites: reorder the chosen sliders.
8. [ ] Camera capture: blocked until a human picks the hardware target.

## Completed (This Batch)
- `0361fc73` detect: the strip decode and axis measurement run on a worker.
- `fb7ddcee` export: `overwrite` is honoured and a batch never reuses a name.
- `3bd9714b` contact sheet: rejected frames start left out; a tile click opens
  its frame.
- `ef0446b5` histogram: bottom band, reversed density, RGB lines, baseline fan.
- `746da799` histogram: the float output is binned before proof; the GPU tail
  emits the density histogram.
- `69d2800e` gpu: startup reads `override.toml` and the saved `gpu_enabled`.
- `0db44310` layout: minimum window and a 1400x900 manifest.
- `d9f5f857` layout: tab overflow menus and a rail-width recipe anchor.
- `8d86e6d6` layout: toolbar collapse order with an overflow menu.
- `1d490c39` frames: a frame select decodes on a worker and drops stale opens.

## Verification
- `cargo check --workspace` passes after the batch.
- Targeted tests pass for frame selection, toolbar layout, narrow manifest,
  GPU preference, histogram data and drawing, contact sheet, export naming and
  Detect.
- `cargo test --workspace` passes: 356 backend library tests and 361 desktop
  tests, plus the integration suites.
- Clippy remains blocked by existing lints, not by this batch.
- The Rust golden captures remain stale; the narrow manifest has no PNGs.

## Constraints / Gotchas
- Never edit `negpy/`; `negpy_core` stays its own workspace.
- Run the app with `cd negpy_egui && cargo run --release --bin negfe`.
- Manual profile `/home/bruno/Dokumenty/NegPy`; roll `vv9`.
- Without `--profile`, Import cannot write and edits save nowhere.
- A section rebuilt in `show` loses its open state on the next frame.
- UX target: the same workflow, controls, order and density as NegPy.
- The histogram strip in a card header uses the filled luma trace; the Analysis
  chart uses the bottom band and RGB lines.
- A frame with a local mask still takes the CPU path. The shader has bindings,
  but the exposure pass receives dummy mask and key textures.
- Contrast Mask and Diffusion also stay on the CPU; their planes need textures.
- The GPU chain reads CPU-measured bounds and runs the finish stage on the CPU.
- Export is CPU-only. `write_export_with_names` shares one name set per batch.
- Detect groups only the axes the worker returned.
- Contact Sheet left-out seeds from `excluded` marks when the dialog opens.
- `GpuTexture::readback` waits on the queue; no pipelining or stage cache exists.
- Camera capture has no hardware adapter in the Rust backend.
- The Scan card's Prescan and Scan as Roll are disabled: no workers.
- The local mask tool returns before it claims a drag when its mode is `Off`.
- Bounds picks still sample the processed image; see the plan before trusting a
  pick on a cropped frame.
- Generated captures stay untracked under `tests/gui/actual/`.
- Session batches are 10 commits; this ledger stays under 120 lines.
- `negpy_egui/AGENTS.md` still says 5 commits and 60 ledger lines. A human must
  reconcile it; the plan and this ledger carry the 10/120 override.
