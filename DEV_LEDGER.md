# Development Ledger
**Last Updated:** 2026-11-16 | **Last Commit:** `e964c99c` backend: add a simulated scanner transport

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up, backend only. Stop before GUI work.
**Source of truth: `negpy_egui/AGENTS.md`**; `MIGRATION_PLAN.md` is advisory.
This session ran 10 backend commits, then the workspace test and a gap audit.

## Completed (This Session)
- `68eb5f34` backend: apply print dye separation and damping.
- `6bda398d` backend: apply print per-layer curve trims.
- `48d01258` backend: write the user gear library to JSON.
- `861d6b02` backend: detect the system monitor ICC profile.
- `bee3dd9c` backend: run CLIP towers over tract-onnx.
- `9439885e` backend: expose the crosstalk and sensor library API.
- `72702035` backend: carry mask and diffusion planes to print.
- `77114769` backend: port scanner params and settings.
- `50fab013` backend: define the scanner transport contract.
- `e964c99c` backend: add a simulated scanner transport.

## Verification (After Commit 10)
- Targeted tests passed per commit; `cargo fmt --all -- --check` clean.
- `cargo test --workspace --no-fail-fast` passed: all binaries, 0 failures.

## Audit (After Commit 10)
- Closed: print dye separation, trims, mask and diffusion planes; gear save;
  monitor ICC detection; CLIP session; crosstalk/sensor API; scanner params,
  settings, transport and simulator.
- **D10:** `image_processor.py` and `preview_manager.py` remain. Missing pure
  helpers: autocrop/meter freeze, splash plus linear one-open path, detection
  downsample and mask resizing. `gpu_engine.py` and WGSL remain.
- **D11:** `services/scanning/service.py` remains: result writing with collision
  search, bounded transient retries, metering and native/per-frame roll glue.
  Hardware transports (`sane`, Plustek, pieusb, nkscan) remain.
- **D12:** capture service orchestration and transports remain. The ONNX runner
  is behind the `tract-onnx` feature, not enabled by default.
- **D13/UI:** session glue, controller and every egui view stay out of scope.

## Next Steps (Ordered)
1. [ ] D10: image processor and preview manager pure helpers.
2. [ ] D11: scanner service result writing and retry policy.
3. [ ] D10: GPU engine and shader wiring.
4. [ ] D12: capture service orchestration over injected hardware.
5. [ ] D13: headless session state, then stop before UI.

## Constraints / Gotchas
- Never edit `negpy/`; `negpy_core` stays its own workspace.
- Fingerprint and export bytes are pinned to Python.
- `negpy-backend` gained `qcms`, `html-escape`, `regex`, `zbus` and optional `tract-onnx`.
- `negpy_egui/target` reached 126 GB. Prune it with
  `cargo clean --profile dev` (keeps release) or `cargo clean -p <crate>`;
  do not wipe the whole directory.
- The workspace test links many integration binaries; on a loaded host it is slow.
- D6: `fff_loader` returns None for SGI LogLuv until RLE/tables land.
