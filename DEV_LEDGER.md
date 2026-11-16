# Development Ledger
**Last Updated:** 2026-11-16 | **Last Commit:** `e6b22b28` library: walk and search the roots

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up, backend only. Stop before GUI work.
**Source of truth: `negpy_egui/AGENTS.md`**; `MIGRATION_PLAN.md` is advisory.
This session ran 10 backend commits, then the workspace test and a gap audit.

## Completed (This Session)
- `2c1ac2f9` geometry: expose the crop-state predicates.
- `e665cb7c` render: resolve arming crop and meter box before the engine.
- `df9b4cfb` retouch: add the detection-plane helpers.
- `f2986308` scanner: add the scan service and per-frame roll.
- `5a3984e5` scanner: write a result through the service.
- `8059307a` capture: orchestrate a triplet over injected hardware.
- `6523c1af` capture: shoot a Blend exposure in one Bulb release.
- `26b5a846` capture: add the plain white-light single.
- `26be165d` capture: add the Scanlight wire codec and settings.
- `e6b22b28` library: walk and search the roots.

## Verification (After Commit 10)
- Targeted tests passed per commit; `cargo fmt --all -- --check` clean.
- `cargo test --workspace --no-fail-fast -j 4` finished; one target failed.
- `base_read::tests::the_oldest_source_is_evicted_first` is a pre-existing
  test-isolation race: its sibling tests share the process-wide cache and a
  parallel `clear()` makes it flake. Reproduced 3/8 on an untouched file.
- Every other target, and the lib alone (`252 passed`), is green.

## Audit (After Commit 10)
- Closed: armed-crop and meter resolution, detection planes, scanner service
  retries and result writing, per-frame roll preview, triplet/Blend/single
  capture, Scanlight protocol and settings, library walk and search.
- **Remaining backend gaps, in order:**
  - D13: `desktop/render_memo.py`, `prefetch_logic.py`, `settings_catalog.py`,
    `sticky.py`, `power_assertion.py`, `startup.py`.
  - D10: `services.rendering.{image_processor,preview_manager,gpu_engine}` ports;
    soft-proof and gamut LUTs; `infrastructure.loaders.lens_metadata` (only the
    warp math is ported, in `features/rgbscan.rs`).
  - D12: `services.assets.semantic_model` download/CLIP glue; scanner hardware
    transports (`sane`, Plustek, pieusb, nkscan).
  - D12: `kernel.system.updater`.
- D6: `fff_loader` returns None for SGI LogLuv until RLE/tables land.
- `base_read` lib tests flake under parallel `clear()`; serialize that suite
  or give each test its own key space.

## Constraints / Gotchas
- Never edit `negpy/`; `negpy_core` stays its own workspace.
- Fingerprints and export bytes are pinned to Python.
- `ScanResult` now lives in `services::scanning::transport`; `writer` re-exports it.
- `ScanError` carries the transient classification; the service retries only that.
- `negpy_egui/target` is large. Prune with `cargo clean --profile dev`.
- Run `cargo test --workspace` only on an idle host.
