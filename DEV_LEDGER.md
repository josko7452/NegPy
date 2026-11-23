# Development Ledger
**Last Updated:** 2026-11-23 14:30 | **Last Commit:** `90d7310d docs: keep the tall set as the Python reference`

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`. This batch moved finish and carrier onto
the GPU tail, made export prefer the GPU seam, fixed uncached render metrics,
added a bounded RAW preview, and refreshed both capture sets with real render
data. The next task is the memory-bounded tiled GPU export.

## Batch Protocol
After commit #10, run the source audit in `MIGRATION_PLAN.md`. Write its
findings here. Run targeted tests for the changed code and
`cargo build --release --bin negfe`. Do not run the full workspace suite. The
audit does not authorize an 11th commit.

## Next Steps (Ordered)
1. [ ] Tiled export (G1-4): add Python's memory-bounded tile path. The
   un-tiled GPU export already matches the CPU oracle on the golden fixture.
2. [ ] Frame switch speed (P2-S1): record the NegPy number on the golden roll
   and compare it with the release NegFe test. The bounded preview halved the
   NegFe open time; do not change the worker before that comparison.
3. [ ] Camera capture: blocked until a human picks the hardware target.

## Completed (This Batch)
- `53a6b516` capture: the golden frame renders from its linear source.
- `9b5e8b35` tests: the narrow guard set is refreshed from its source.
- `90d7310d` docs: the tall set stays the Python reference.
- `b4affaef` frame: IR removal keeps the full decode.
- `533d6bab` loaders: a RAW preview bins without a full demosaic.
- `db89aa4d` frame: the golden roll switch is measured.
- `601c0741` gpu: the print finish runs on the device.
- `4fa64f0d` export: the GPU chain is preferred when a device opens.
- `7ec50bce` engine: an uncached render keeps its metrics and crop.

## Verification
- Targeted tests pass for the capture fixture/launch path, the bounded RAW
  preview, the finish/carrier GPU parity, export reporting and the render
  engine's uncached metrics.
- `cargo build --release --bin negfe` passes after the batch.
- The refreshed narrow captures pass the 0.80 layout gate; tone and finish
  measure 0.879 and 0.884 against the previous tracked set.
- The full-resolution GPU export oracle passes on the golden fixture. The
  tiled path remains open.
- Clippy remains blocked by existing lints, not by this batch.

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
- The GPU tail runs finish and carrier; carrier and vignette chain parity pass.
- Export prefers the GPU per frame and falls back to the CPU with a reason.
- A frame switch in NegFe is slower than in NegPy; the bounded preview halves
  the decode. Measure NegPy before changing the worker again.
- Detect groups only the axes the worker returned.
- Contact Sheet left-out seeds from `excluded` marks when the dialog opens.
- `GpuTexture::readback` waits on the queue; no pipelining or stage cache exists.
- Camera capture has no hardware adapter in the Rust backend.
- The Scan card's Prescan and Scan as Roll are disabled: no workers.
- The local mask tool returns before it claims a drag when its mode is `Off`.
- Bounds picks still sample the processed image; see the plan before trusting a
  pick on a cropped frame.
- The capture manifest records `source_folder` and `source_profile`; both are
  needed to regenerate either set.
- Generated captures stay untracked under `tests/gui/actual/`.
- Session batches are 10 commits; this ledger stays under 120 lines.
- `negpy_egui/AGENTS.md` still says 5 commits and 60 ledger lines. A human must
  reconcile it; the plan and this ledger carry the 10/120 override.
