# Development Ledger
**Last Updated:** 2026-11-17 | **Last Commit:** `32c5d156` export: choose the container encoder

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up, backend only. Stop before GUI work.
**Source of truth: `negpy_egui/AGENTS.md`**; `MIGRATION_PLAN.md` is advisory.
This session runs a second 10-commit backend batch, then the audit and workspace test.

## Completed (Recent)
- `6da70651` session: memo the last render per frame.
- `1e973914` display: bake soft-proof and gamut tables.
- `7d354e2d` session: resolve a frame's composite parts.
- `6e333a14` assets: fetch the CLIP model atomically.
- `c1b10f82` render: assemble composite previews.
- `f8e1e6ca` export: convert pixels and profiles.
- `32c5d156` export: choose the container encoder.
- In flight: export write path.

## Verification
- Targeted tests: `workspace_session`, `asset_config`, semantic download,
  `preview_composites`, `preview_manager`, export pixel, encode and service unit tests.
- `cargo fmt --all -- --check` clean.
- Workspace test runs after commit 10 of this batch.

## Audit (Before This Batch)
- Closed: render memo, settings catalog, sticky store, power assertion, lens metadata,
  updater, preview cache/prefetch/detection, proof and gamut LUTs.
- **D10:** `services.rendering.gpu_engine` and all WGSL shaders remain unported.
- **D10:** `ImageProcessor` remains partial: full export render/encode path, source
  correction caches, CMYK per-pixel proof flags, and u16 color management remain.
- **D10:** preview manager remains partial: splash and one-open paths, RGB/HDR/stitch
  merges, and camera-WB/lens-correction decode glue remain.
- **D12:** `services.assets.semantic_model` download glue remains; CLIP inference itself
  is ported behind `tract-onnx`.
- **D12:** scanner hardware transports remain (`sane`, Plustek, pieusb, nkscan).
- **D6:** `fff_loader` returns None for SGI LogLuv until RLE/tables land.
- GUI work remains out of scope.

## Constraints / Gotchas
- Never edit `negpy/`; `negpy_core` stays its own workspace.
- Fingerprints and export bytes are pinned to Python.
- The 10-commit request supersedes the 5-commit checkpoint in `negpy_egui/AGENTS.md`.
- Headless desktop helpers live under `session/`, not as 1:1 module filenames.
- `c2df6270` added the catalog; `ada50a63` completed missing Paper Response, Retouch
  and Finishing rows needed by the sticky migrations.
- `negpy_egui/Cargo.lock` gained `objc2-foundation` for the macOS power assertion.
