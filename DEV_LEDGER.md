# Development Ledger
**Last Updated:** 2026-11-17 | **Last Commit:** `pending` scanning: register the scanner backends

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up, backend only. Stop before GUI work.
**Source of truth: `negpy_egui/AGENTS.md`**; `MIGRATION_PLAN.md` is advisory.
This session completed a second 10-commit backend batch, the audit and the workspace test.

## Completed (Recent)
- `pending` scanning: register the scanner backends.
- `1e973914` display: bake soft-proof and gamut tables.
- `7d354e2d` session: resolve a frame's composite parts.
- `6e333a14` assets: fetch the CLIP model atomically.
- `c1b10f82` render: assemble composite previews.
- `f8e1e6ca` export: convert pixels and profiles.
- `32c5d156` export: choose the container encoder.
- `7f4ea22e` export: write a finished frame.
- `c2b0a151` render: memo the corrected export source.
- `925bc048` render: paint a reduced-page splash.
- `1dd59e6b` display: prove a CMYK paper per pixel.

## Verification
- Targeted tests passed per commit; `cargo fmt --all -- --check` clean.
- `cargo test --workspace --no-fail-fast -j 4` passed after the audit.

## Audit (After Commit 10)
- Closed: workspace session, asset resolvers, CLIP download, RGB/HDR/stitch previews,
  export pixels/profiles/encoders/write path, source cache, reduced-page splash,
  per-pixel CMYK proof.
- **D10:** `services.rendering.gpu_engine` and all WGSL shaders remain unported.
- **D10:** `ImageProcessor` remains partial: full-res render glue, retouch/source bake
  cache coordination, and JPEG/RAW embedded-preview extraction remain.
- **D10:** `preview_manager` still lacks the one-open splash/linear call and camera-WB
  decode glue; RGB/HDR/stitch assembly now exists.
- **D10:** a finished export has no metadata embed step yet; the primitives exist.
- **D12:** `services.assets.semantic_model` still lacks real ONNX tokenizer/model file
  download verification; the download writer exists.
- **D12:** scanner hardware transports remain (`sane`, Plustek, pieusb, nkscan).
- **D6:** embedded-thumbnail extraction from camera RAW and the bounded DNG preview
  remain; the Flextight LogLuv RLE/tables path is ported and tested.
- JPEG XL and WebP export need pure-Rust encoders; they report a block reason.
- GUI work remains out of scope.

## Constraints / Gotchas
- Never edit `negpy/`; `negpy_core` stays its own workspace.
- Fingerprints and export bytes are pinned to Python.
- The 10-commit request supersedes the 5-commit checkpoint in `negpy_egui/AGENTS.md`.
- Headless desktop helpers live under `session/`, not as 1:1 module filenames.
- `c2df6270` added the catalog; `ada50a63` completed missing Paper Response, Retouch
  and Finishing rows needed by the sticky migrations.
- `negpy_egui/Cargo.lock` gained `objc2-foundation` for the macOS power assertion.
