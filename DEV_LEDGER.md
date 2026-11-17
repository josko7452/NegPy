# Development Ledger
**Last Updated:** 2026-11-17 | **Last Commit:** `e5fc7aad` capture: decode sensor RGB for metering

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up, backend only. Stop before GUI work.
**Source of truth: `negpy_egui/AGENTS.md`**; `MIGRATION_PLAN.md` is advisory.

## Completed (Recent)
- `7605867d` assets: scan crosstalk profile files.
- `303cb18c` metadata: read a source EXIF block.
- `4b0803d3` export: attach an Exif IFD to TIFF.
- `33d95ab4` export: embed a metadata plan.
- `29765ca1` export: render a file at full resolution.
- `1a091aca` loaders: read an embedded RAW preview.
- `63991661` render: decode a preview on camera WB.
- `6e4276b5` export: encode a lossless WebP.
- `85706b4f` scanning: register the scanner backends.
- `e5fc7aad` capture: decode sensor RGB for metering.

## Verification
- Targeted tests passed per commit; `cargo fmt --all -- --check` clean.
- Workspace test run after the audit (result below).

## Audit (After Commit 10)
- Closed this batch: crosstalk profile TOML store, source EXIF read (JPEG/TIFF/JXL),
  TIFF Exif IFD attach, metadata embed plan and write path, full-res export render,
  embedded RAW preview, preview camera WB and one-open splash, lossless WebP,
  scanner backend registry, sensor-RGB metering decode.
- **D10:** `services.rendering.gpu_engine` and all WGSL shaders remain unported.
- **D10:** `ImageProcessor` is still split across `import`, `render`, `export`; the
  source-bake cache coordination and diptych join remain.
- **D10:** `cam_xyz` and `camera_wb` are not yet threaded through `PipelineContext`;
  `render_white_space` and the transfer base still read defaults, and `RawlerDecoder`
  always applies as-shot WB, so `linear_raw` parity is open.
- **D12:** `semantic_model` still needs real ONNX tokenizer/model verification beyond
  the download writer.
- **D12:** scanner hardware transports (`sane`, Plustek, pieusb, nkscan) remain.
- JPEG XL export and lossy WebP report a block reason; both need encoders.
- GUI work remains out of scope.

## Constraints / Gotchas
- Never edit `negpy/`; `negpy_core` stays its own workspace.
- Fingerprints and export bytes are pinned to Python.
- The 10-commit request supersedes the 5-commit checkpoint in `negpy_egui/AGENTS.md`.
- Headless desktop helpers live under `session/`, not as 1:1 module filenames.
- `dump_exif` now counts the empty Exif/GPS directories in its extra-data offset.
