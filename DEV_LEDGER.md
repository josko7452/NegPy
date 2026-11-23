# Development Ledger
**Last Updated:** 2026-11-23 09:50 | **Last Commit:** `gpu: cover the measured bounds context`

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`. This batch bound the carrier tables,
added a GPU export seam and its parity oracle, shows a processing line, tracks
the narrow capture set and tests the rebate tone table. The next task measures
why a frame switch is slower than in NegPy. The plan is the active register;
completed history is in `MIGRATION_PLAN_ARCHIVE.md`.

## Batch Protocol
After commit #10, run the source audit in `MIGRATION_PLAN.md`. Write its
findings here, then run `cargo check --workspace` and
`cargo test --workspace`. The audit does not authorize an 11th commit.

## Next Steps (Ordered)
1. [ ] Export: close the remaining full-resolution GPU/CPU parity gap and add
   the tiled GPU path (G1-4). The ignored oracle is
   `a_full_resolution_gpu_export_matches_the_cpu`.
2. [ ] Frame switch speed (P2-S1): first measure NegPy against NegFe over the
   golden "test" roll. Then find the slower stage and fix it. Do not change the
   worker before the measurement.
3. [ ] Finish: call the carrier-capable finish shader from the GPU tail and keep
   the CPU fallback until its parity holds (G1-5).
4. [ ] Evidence: decide whether the tall set follows the narrow set (H1-6,
   D1-4). The narrow Rust set is tracked in `golden/narrow/`.
5. [ ] Camera capture: blocked until a human picks the hardware target.

## Completed (This Batch)
- `gpu: cover the measured bounds context`: the measured-bounds context has a direct test.
- `a23ce8d0` canvas: a failed open clears the busy line.
- `a4efac3f` finish: the rebate tone table matches the core carrier.
- `c785ca94` tests: the narrow Rust capture set is tracked.
- `30374e1b` canvas: a processing line shows while a frame waits.
- `aca27397` gpu: measured bounds reach the export context.
- `69186ffc` export: a GPU render entry point and its ignored parity oracle.
- `7174a9ca` finish: the carrier tables size from their input.
- `2ca989c7` finish: the shader runner accepts carrier tables.

## Verification
- `cargo check --workspace` passes after the batch.
- Targeted tests pass for finish tables and carrier parity, export reporting,
  the narrow capture manifest, and the processing state.
- `cargo test -p negpy-backend --lib` passes: 359 tests. `cargo test -p
  negpy-desktop --lib` passes: 372 tests. The uncached workspace integration
  link stalled in `rust-lld`, so the integration suites were not rerun.
- The export parity oracle is ignored until G1-4 is fixed.
- Clippy remains blocked by existing lints, not by this batch.
- The narrow Rust set is tracked under `tests/gui/golden/narrow/`. The tall
  Python set remains the binding reference.

## Constraints / Gotchas
- Never edit `negpy/`; `negpy_core` stays its own workspace.
- Run the app with `cd negpy_egui && cargo run --release --bin negfe`.
- Manual profile `/home/bruno/Dokumenty/NegPy`; roll `vv9`.
- Without `--profile`, Import cannot write and edits save nowhere.
- A section rebuilt in `show` loses its open state on the next frame.
- UX target: the same workflow, controls, order and density as NegPy.
- The histogram strip in a card header uses the filled luma trace; the Analysis
  chart uses the bottom band and RGB lines.
- Local masks, Contrast Mask and Diffusion bind real GPU planes. The masked
  tail and masked exposure parity tests pass.
- The GPU chain reads CPU-measured bounds and runs the finish stage on the CPU.
- Export still uses the CPU writer. The GPU seam exists, but its full-resolution
  oracle is ignored until the remaining pixel gap closes.
- A frame switch in NegFe is slower than in NegPy. The next session measures it
  over the golden "test" roll before changing the worker.
- Detect groups only the axes the worker returned.
- Contact Sheet left-out seeds from `excluded` marks when the dialog opens.
- `GpuTexture::readback` waits on the queue; no pipelining or stage cache exists.
- Camera capture has no hardware adapter in the Rust backend.
- The Scan card's Prescan and Scan as Roll are disabled: no workers.
- The local mask tool returns before it claims a drag when its mode is `Off`.
- Bounds picks still sample the processed image; see the plan before trusting a
  pick on a cropped frame.
- The capture manifest records `source_folder` and `source_profile`; the narrow
  set needs both to regenerate.
- Generated captures stay untracked under `tests/gui/actual/`.
- Session batches are 10 commits; this ledger stays under 120 lines.
- `negpy_egui/AGENTS.md` still says 5 commits and 60 ledger lines. A human must
  reconcile it; the plan and this ledger carry the 10/120 override.
