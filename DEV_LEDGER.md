# Development Ledger
**Last Updated:** 2026-11-17 | **Last Commit:** `1e973914` display: bake soft-proof and gamut tables

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up, backend only. Stop before GUI work.
**Source of truth: `negpy_egui/AGENTS.md`**; `MIGRATION_PLAN.md` is advisory.
This session ran the requested 10 backend commits, then the audit and workspace test.

## Completed (This Session)
- `6da70651` session: memo the last render per frame.
- `05c0f9f6` session: select the neighboring strip frames.
- `c2df6270` session: catalogue copyable settings rows.
- `ada50a63` session: carry chosen settings to new frames.
- `df7bdcac` session: hold power for unattended work.
- `d1a9d28a` session: recover an unwritable data folder.
- `62b60a8e` loaders: read embedded lens correction metadata.
- `549c58f3` kernel: find and stage application updates.
- `7c7bc908` render: manage decoded previews in one place.
- `1e973914` display: bake soft-proof and gamut tables.

## Verification (After Commit 10)
- Targeted tests passed per commit; `cargo fmt --all -- --check` clean.
- The audit below was rerun after commit 10.
- `cargo test --workspace --no-fail-fast -j 4` passed on the final rerun.
- One earlier workspace run reported only the known `base_read` shared-cache race;
  its lib target passed standalone.

## Audit (After Commit 10)
- Closed: session render memo, strip neighbors, settings catalog, sticky store and
  migrations, power assertion, startup preflight, lens metadata readers, updater,
  preview-manager cache/prefetch/detection helpers, proof and gamut LUTs.
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
