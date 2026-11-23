# Archived Migration Plan

**Archived:** 2026-11-22. This file holds the prior migration plan and its batch
audits. The active register is `MIGRATION_PLAN.md`.

---

# NegPy `negpy` → `negpy_egui` Migration Plan

Pure-Rust port of the entire Python layer (`negpy/`, 427 modules) into a new
**`negpy_egui/`** workspace that links `negpy_core` directly, with **zero
breaking changes** to `negpy/` or to `negpy_core`'s PyO3 bindings.

`negpy_egui/` is the Rust-workspace folder name chosen only to avoid clashing
with the existing `negpy/` Python package. It is **its own** Cargo workspace;
`negpy_core` stays exactly where it is (its own workspace, maturin target).

---

## SOURCE OF TRUTH

**`negpy_egui/AGENTS.md` is the ONLY source of truth for this plan.** It is the
authority on scope, layering, upstream compatibility, architecture/performance
rules, commit style, the commit batch stop, and ledger keeping. `MIGRATION_PLAN.md` is
a *derived, advisory* document: it exists to save discovery time, not to override
the rules.

- **Precedence:** On any conflict — scope, layer order, crate layout, naming,
  commit rules — **`negpy_egui/AGENTS.md` wins.** Read it at the start of every
  session before doing anything else.
- **If they disagree, do not silently pick one.** Follow `negpy_egui/AGENTS.md`,
  then record the divergence in `DEV_LEDGER.md` under "Constraints / Gotchas" so a
  human can reconcile this plan.
- **`negpy_egui/AGENTS.md` is expected to change.** When it does, this plan is stale
  until updated. A rule change that is not reflected there is a plan edit only.
- The repo-root `AGENTS.md` (symlink to `CLAUDE.md`) governs the **existing Python
  repo** and does not apply to new `negpy_egui/` development; `negpy_egui/AGENTS.md`
  is the scoped authority for the Rust port.

### Reconciliation status (re-checked)

AGENTS.md was re-checked against this plan; **there are no remaining
contradictions.** Details:

- **Crate layout — resolved.** `negpy_egui/AGENTS.md` no longer states a
  single-crate layout (the "Core Objective" / `src/services` / `src/ui` wording is
  gone) and rule 2 is now topology-neutral ("keep all new Rust code inside
  `negpy_egui/`"). The plan's two-crate layout (`backend/`, `desktop/`) therefore
  **complies**. Advisory detail only, not a rule conflict.
- **Verification — consistent.** AGENTS.md says
  `cargo check --workspace && cargo test --workspace`; the plan's `negpy_egui/`
  workspace satisfies this exactly.
- **`DEV_LEDGER.md` location — resolved.** AGENTS.md says "repository root"; the
  plan places it at the repo root (`NegPy/DEV_LEDGER.md`), which satisfies the rule
  literally. (If development is fully scoped to `negpy_egui/`, a ledger there would
  also be acceptable, but the repo root is what AGENTS.md names.)

### Advisory notes (plan guidance, not binding)

These are choices the plan makes that AGENTS.md leaves open; they are **not**
conflicts and may be changed without touching the source of truth.

1. **Two crates vs one.** The plan splits `backend/` (headless) from `desktop/`
   (egui) to enforce the "no GUI deps in backend" boundary structurally. A single
   crate would work but loses that compiler-enforced guarantee.
2. **No repo-root `Cargo.toml`.** The plan keeps `negpy_egui/` as its own
   workspace, with `negpy_core` as a path dependency (not a member), so
   `negpy_core/Cargo.lock`, `flake.nix` and `[tool.uv].cache-keys` stay untouched.
   This is a plan decision, not an AGENTS.md requirement.
3. **`negpy_core` edits in Layer 1 (interpretation call).** AGENTS.md rule 1.2
   says "keep all new Rust code inside `negpy_egui/`". Layer 1 proposes *additive*
   `pub` promotions in `negpy_core/src/*.rs` (Section 3). Read strictly, any edit
   to `negpy_core/` is out of bounds; read as "new feature code", an additive
   `pub` promotion that keeps maturin builds identical is fine. The audit found the
   pure kernels are **already `pub`** and the PyO3 wrappers already delegate, so
   Layer 1 should be a **verify-only** pass first; if a promotion is genuinely
   required, treat it as the sole exception and confirm with the human before
   committing (scope `core:`, per Section 8.1). Do not batch `negpy_core` edits
   into `negpy_egui` commits.

---

## 0. Repo Facts (verified)

- Python package `negpy/` = **427** modules: `kernel/` 19, `domain/` 7,
  `infrastructure/` 67, `services/` 76, `features/` 95, `desktop/` 162.
- Rust extension `negpy_core` is built by **maturin** (`negpy_core/pyproject.toml`,
  `build-backend = "maturin"`, `features = ["python"]`), ships its **own**
  `Cargo.lock` + `rust-toolchain.toml` (1.99), and is therefore its own workspace.
  PyO3 + numpy are optional deps behind `python = ["dep:pyo3", "dep:numpy"]`.
- `negpy_core/src/*.rs` = pure algorithms; `negpy_core/src/py/*.rs` = thin PyO3
  wrappers. No root `Cargo.toml` exists.
- 33 WGSL shaders under `negpy/features/**/shaders/`.
- **Layering is clean:** no module in `kernel/ domain/ features/ infrastructure/
  services/` imports `negpy.desktop` (verified). `desktop/` is a pure top layer.

### 0a. Package-level dependency reality (why crates ≠ 1:1 folders)

Internal `negpy.*` imports collapse to **one strongly-connected component** at
top-level-package granularity. Concrete bidirectional edges:

| Edge | Forward | Back |
|---|---|---|
| `services` ↔ `infrastructure` | 51 | 2 |
| `services` ↔ `features` | 168 | 1 |
| `features` ↔ `domain` | 49 | 15 |
| `features` ↔ `kernel` | 37 | 7 |
| `infrastructure` ↔ `features` | 13 | 4 |
| `infrastructure` ↔ `kernel` | 38 | — |
| `domain` ↔ `kernel` | 6 | 3 |

**Conclusion:** `kernel`, `domain`, `features`, `infrastructure`, `services`
cannot be five Rust *crates* — Cargo rejects the cycle. Only **`desktop`** is a
cleanly separable crate (zero incoming edges from anywhere). The layout below
keeps the exact 1:1 **folder** mapping inside one backend crate and promotes
`desktop` to its own crate.

---

## 1. External Python Dependency Audit (`pyproject.toml`)

### 1a. Runtime third-party packages → crate replacement

| Python pkg (version) | Where used (representative modules) | Pure-Rust replacement crate | Notes |
|---|---|---|---|
| `rawpy==0.27.0` | `infrastructure/loaders/rawpy_loader.py`, `capture/raw_demosaic.py`, `display/color_spaces.py`, `features/rgbscan/logic.py`, `services/rendering/{image_processor,preview_manager}.py`, `services/export/{linear_output,contact_sheet_roll}.py`, `services/assets/{thumbnails,migrations/flatfield_layout}.py` | **`rawler`** (pure Rust, LibRaw-compatible RAW demosaic + EXIF) | Bit-parity of demosaic output is the main risk; expose behind a `RawDecoder` trait so `libraw-sys` can substitute. |
| `numpy==2.4.4` | 122 modules (feature logic + most desktop) | **`ndarray`** (+ `rayon`) | Bulk pixel math already lives in `negpy_core` kernels; use `ndarray` only for array *plumbing*. |
| `tifffile==2026.6.1` | `infrastructure/loaders/{tiff_loader,fff_loader,nef_loader,helpers,lens_metadata,memory}.py`, `features/metadata/writer.py`, `services/export/{encoders,linear_output,frame_merge,contact_sheet_roll}.py`, `services/scanning/writer.py`, `infrastructure/scanners/plusteck_backend.py` | **`tiff`** (image-tiff) | Multi-page, 16-bit, IR-plane, EXIF tag read/write parity. |
| `imagecodecs==2026.6.6` | `infrastructure/loaders/{jxl_boxes,jxl_loader}.py`, `services/export/{encoders,linear_output}.py`, `services/rendering/image_processor.py` | **`jxl-oxide`**, **`zune-jpeg`**, **`png`**, **`image-webp`**, **`flate2`/`zstd`/`lz4_flex`** | JXL-in-TIFF deliberately unused; standalone lossless JXL only. |
| `imageio==2.37.3` | `infrastructure/loaders/{jpeg_loader,tiff_loader}.py` | **`image`** (or direct decoders) | Thin read/write layer. |
| `jinja2==3.1.6` | `services/export/templating.py`, `services/scanning/templating.py` | **`minijinja`** | `docs/TEMPLATING.md` is the parity spec. |
| `pillow==12.2.0` | 29 modules: `infrastructure/display/{icc_lut,color_mgmt}.py`, `loaders/memory.py`, `desktop/controller.py`, `desktop/view/**`, `features/metadata/*`, `services/export/*` | **`image`** + **`imageproc`**; ICC via **`qcms`** (pure Rust) or **`lcms2`** | PIL `ImageCms` used only for ICC transforms; `qcms` matches the 3D-LUT transform in `display/icc_lut.py`. |
| `pyqt6==6.11.0` | 136 desktop modules | **`egui`/`eframe`** + **`egui_extras`** + **`rfd`** | Layer 3 UI. |
| `pyqt6-charts==6.11.0` | `desktop/view/widgets/charts.py`, `chart_calibration_dialog.py` | **`egui_plot`** | Charts drawn with `egui::Painter`. |
| `qtawesome==1.4.2` | 36 desktop modules (icons) | **`egui-phosphor`** | Icon-name → glyph map. |
| `piexif==1.1.3` | `features/metadata/{exif_read,capture,writer,resolution}.py`, `services/export/*`, `desktop/session.py` | **`kamadak-exif`** (read) + **`img-parts`** (write) | `kamadak-exif` is read-only. |
| `onnxruntime==1.23.2` | `services/assets/semantic_model.py` (1 module) | **`ort`** or **`tract`** | `ort` gives model parity; `tract` is pure-Rust. |
| `wgpu==0.31.1` (PyPI) | `infrastructure/gpu/{device,resources,shader_loader,cffi_warm}.py`, `services/rendering/gpu_engine.py`, `desktop/view/canvas/gpu_widget.py` | **`wgpu`** (Rust crate, same API) + **`egui_wgpu`** + **`naga`** | Direct port; 33 WGSL shaders carry over verbatim. |
| `pyserial>=3.5` | `infrastructure/capture/{lento_dock,scanlight}.py` | **`serialport`** | |
| `pyopticfilm>=1.3.9` (opt) | `infrastructure/scanners/plustek_backend.py` | **`rusb`**/**`nusb`** | Hardware-protocol port. |
| `pieusb>=0.3.7` (opt) | `infrastructure/scanners/pieusb_backend.py` | **`rusb`**/**`nusb`** | |
| `nkscan>=0.18` (opt) | `infrastructure/scanners/{nkscan_backend,nkscan_log}.py` | **`nkscan`** (itself a Rust crate) | Consume directly. |
| `python-sane>=2.9` (opt) | `infrastructure/scanners/sane_backend.py` | **`sane`** | Host SANE plugins still loaded externally. |
| `gphoto2>=2.5` (opt) | `infrastructure/capture/gphoto.py` | **`gphoto2`** | macOS/Linux only. |
| `usb` | `infrastructure/scanners/{pieusb,plustek}_backend.py` | **`rusb`**/**`nusb`** | |
| `rendercanvas` | `desktop/view/canvas/gpu_widget.py` | **`egui_wgpu`** surface | Replaced by egui's wgpu callback. |
| `cffi` | `infrastructure/gpu/cffi_warm.py` | *(dropped)* | Only warmed wgpu FFI handles. |

### 1b. Python stdlib replaced by crates

| Stdlib | Where | Replacement |
|---|---|---|
| `sqlite3` | `infrastructure/storage/repository.py` (+8 `services/assets/migrations/*.py`) | **`rusqlite`** (bundled) — schema must match `edits.db`/`settings.db` exactly |
| `json` / `tomllib` | sidecars, presets, `services/assets/toml_cache.py`, `kernel/system/override.py` | **`serde_json`** / **`toml`** — byte-parity with Python output is a hard requirement |
| `hashlib` | `domain/tokens.py`, `services/assets/thumbnail_fingerprint.py`, migrations | **`sha2`** / **`blake3`** |
| `threading`/`concurrent` | workers, `kernel/caching/manager.py` | **`std::thread`** + **`crossbeam-channel`** + **`rayon`** |
| `urllib` | `kernel/system/version.py` (updater) | **`ureq`** |
| `ctypes` | `kernel/system/{memory,paths,updater}.py`, `desktop/power_assertion.py`, `features/metadata/fsdate.py` | **`windows-sys`** / **`libc`** |
| `tempfile`/`os`/`pathlib` | everywhere | **`std::fs`**, **`tempfile`**, **`directories`** |

---

## 2. Internal Module Dependency Graph (Bottom-Up Order)

Levels by longest-path topological sort of internal `negpy.*` imports.
**427 modules, max depth 25.**

> The 67 modules importing `negpy_core` call its **public pure `pub fn`s** — never
> the PyO3 wrappers. The Rust port calls the same functions with `&[f32]` instead
> of numpy arrays.

### Layer 0 — Leaf utilities, constants, data models (no internal imports)

Level 0 (103 modules): `domain.{types,tokens,migrations}`; `features.*/models`;
`features.metadata.{capture,exif_read,fsdate}`; `kernel.system.{memory,timing,user_directory,version}`;
`infrastructure.{capture.{base,protocol},display.icc_profile,loaders.{constants,ir_planes,jxl_boxes,logluv,memory},scanners.{params,result},simulated.{images,lento_dock}}`;
`services.assets.{capture_modes,clip_tokenizer,naming,search,toml_cache}`,
`services.capture.{focus_meter,live_stats,presets}`, `services.film_edge.{barcodes,geometry}`,
`services.scanning.templating`; `features.process.capture_color`.

Level 1 (20) → Level 2 (13): `kernel.system.{i18n,logging,paths,override}`,
`kernel.image.{logic,validation}`, `kernel.caching.{logic,manager}`,
`infrastructure.scanners.base`, `infrastructure.display.icc_lut`,
`services.assets.library`, `features.{hdr.models,lens.warps,stitch.models,finish.logic,stitch.logic}`,
`infrastructure.loaders.lens_metadata`, `services.film_edge.{edge,layouts}`.

### Layer 1 — Core decoupling (`negpy_core`) — see Section 3.

### Layer 2 — Headless services & I/O (pure Rust, unit-tested)

1. **System/services base (L3–L6):** `kernel.system.{text,updater}`, `kernel.image.logic`,
   `services.assets.migrations.{cast_removal,source_color}`,
   `features.metadata.{gear_logic,payload,xmp}`, `features.process.models`,
   `infrastructure.gpu.{device,resources,shader_loader,cffi_warm}`.
2. **Serialization & storage (L5–L7):** `domain.models`, `kernel.system.config`,
   `domain.interfaces`, `infrastructure.storage.{repository,local_asset_store}`,
   `services.assets.{sidecar,presets,rolls,toml_cache}` (exact JSON/TOML schemas).
3. **Loaders (L7–L10):** `infrastructure.loaders.{helpers,memory,nef_loader,fff_loader,pakon_loader,noritsu_loader,jxl_loader,rawpy_loader,tiff_loader,jpeg_loader,source_encoding,source_kind,factory}`.
4. **Feature kernels (L4–L11):** `process.*`, `exposure.*`, `geometry.*`, `lab.*`,
   `local.logic`, `retouch.{logic,clone}`, `toning.{logic,processor}`,
   `finish.{logic,processor}`, `flatfield.logic`, `hdr.logic`, `lens.{logic,warps,models}`,
   `rgbscan.logic`, `stitch.logic`, `transparency.{logic,processor}`,
   `cyanotype.{logic,processor}`, `lith.{logic,processor}`, `sabattier.{logic,processor}`,
   `altprocess.models`, `metadata.{writer,resolution}`, `pickbetter.logic`.
5. **Rendering engine (L7–L14):** `services.rendering.{base_read,lens,preview_cache,prefetch_policy,source_identity,engine,image_processor,gpu_engine,preview_manager}`,
   `services.maps`, `infrastructure.display.{color_mgmt,color_spaces,monitor_profile}`.
6. **Export/scan/templating (L6–L13):** `services.export.{print,templating,encoders,linear_output,contact_sheet,contact_sheet_layout,contact_sheet_roll,frame_merge}`,
   `services.scanning.{service,writer,templating}`, `services.film_edge.{paint,layouts,barcodes,edge}`.
7. **Assets & headless desktop glue (L7–L15):** `services.assets.*`,
   `services.capture.*`, `services.view.*`, `domain.session`,
   `desktop.{render_memo,prefetch_logic,auto_sliders,settings_catalog,sticky,power_assertion,startup,windows_data_dialog}`.

### Layer 3 — `egui` desktop frontend (crate `negpy-desktop`)

Port bottom-up: `view.styles.*` → `view.widgets.*` → `view.canvas.*` +
`view.sidebar.*` + shortcuts → dialogs → `file_actions`/`main_window` →
`controller` → `main`. (Full module checklist in Section 2's ordered list below.)

### Ordered checklist (bottom → top)

```
L0  domain/{types,tokens,migrations} · features/*/models · metadata/{capture,exif_read,fsdate}
L0  kernel/system/{memory,timing,user_directory,version} · loaders/{constants,ir_planes,jxl_boxes,logluv,memory}
L0  scanners/{params,result} · capture/{base,protocol} · display/icc_profile · services/assets/{naming,search,toml_cache}
L1  kernel/system/{i18n,logging,paths,override} · kernel/image/{logic,validation} · kernel/caching/{logic,manager}
L1  scanners/base · display/icc_lut · assets/library · features/{hdr.models,lens.warps,stitch.models}
L2  system/{text,updater} · gpu/{device,resources,shader_loader} · migrations/{cast_removal,source_color}
L2  domain/models · system/config · domain/interfaces · storage/{repository,local_asset_store}
L2  assets/{sidecar,presets,rolls,toml_cache} · loaders/helpers
L3  features/process/* · features/exposure/* · features/geometry/* · features/lab/* · features/local/*
L3  features/{retouch,toning,finish,flatfield,hdr,lens,rgbscan,stitch,transparency,cyanotype,lith,sabattier}/*
L4  loaders/* · services/rendering/* · services/maps · display/{color_mgmt,color_spaces,monitor_profile}
L5  services/export/* · services/scanning/* · services/film_edge/*
L6  services/assets/* · services/capture/* · services/view/* · domain/session
L6' desktop/{render_memo,prefetch_logic,auto_sliders,settings_catalog,sticky,power_assertion,startup,windows_data_dialog}
L7  ui/styles/* · ui/widgets/* · ui/shortcuts/*
L8  ui/canvas/* · ui/sidebar/* · ui/dialogs/*
L9  ui/{file_actions,frame_merge_action,sidecar_action,main_window,mac_window,mac_menu_bar}
L10 desktop/{controller,main,session}
```

---

## 3. `negpy_core` PyO3 Decoupling Audit

**Finding: almost nothing needs extraction.** Already two-layer: `src/<name>.rs`
holds pure `pub fn` kernels; `src/py/<name>.rs` holds `#[pyfunction]` wrappers
that only validate numpy dtype/shape/contiguity, call `super::detach(py, n, …)`,
and delegate.

Example (`src/py/stats.rs`):
```rust
#[pyfunction]                                  // PyO3 wrapper (stays UNCHANGED)
fn percentiles(py: Python<'_>, values: PyReadonlyArray1<'_, f32>, qs: Vec<f64>) -> PyResult<Vec<f32>> {
    ...
    super::detach(py, values.len(), || stats::percentiles(values, &qs))   // <-- pure
}
```
`negpy_egui` calls the right-hand side directly:
```rust
// negpy_core/src/stats.rs (already pub — no change needed)
pub fn percentiles(values: &[f32], qs: &[f64]) -> Option<Vec<f32>> { ... }
```

### 3a. Every `py/*.rs` wrapper and its already-pure counterpart

| Wrapper (`src/py/…`) | Wrappers | Pure target |
|---|---|---|
| `analysis.rs` | 36 | `crate::analysis::*` (38 `pub fn`) |
| `retouch.rs` | 25 | `crate::retouch::*` (37) |
| `convert.rs` | 20 | `crate::convert::*` (18) |
| `capture.rs` | 13 | `crate::capture::*` (13) |
| `contours.rs` | 11 | `crate::contours::*` (5) + `crate::shapes` (15) |
| `color.rs` | 11 | `crate::color::*` (13) |
| `stitch.rs` | 10 | `crate::stitch::*` (10) |
| `resize.rs` | 9 | `crate::resize::*` (12) |
| `zones.rs` | 9 | `crate::zones::*` (10) |
| `lab.rs` | 8 | `crate::lab::*` (11) |
| `loaders.rs` | 7 | `crate::loaders::*` (8) |
| `histogram.rs` | 7 | `crate::histogram::*` (17) |
| `warp.rs` | 7 | `crate::warp::*` (10) |
| `flatfield.rs` | 6 | `crate::flatfield::*` (9) |
| `hdr.rs` | 6 | `crate::hdr::*` (6) |
| `skew.rs` | 6 | `crate::skew::*` (7) |
| `filter.rs` | 5 | `crate::filter::*` (8) |
| `gray.rs` | 5 | `crate::gray::*` (6) |
| `sensor.rs` | 5 | `crate::sensor::*` (8) |
| `components.rs` | 4 | `crate::components::*` (2) + `crate::distance` (4) |
| `autocrop.rs` | 3 | `crate::autocrop::*` (4) |
| `altprocess.rs` | 3 | `crate::altprocess::*` (7) |
| `bounds.rs` | 2 | `crate::bounds::*` (3) |
| `display.rs` | 4 | `crate::display::*` (5) |
| `fade.rs` | 4 | `crate::fade::*` (7) |
| `finish.rs` | 2 | `crate::finish::*` (3) |
| `jpeg.rs` | 2 | `crate::jpeg::encode` (1) |
| `lens.rs` | 4 | `crate::lens::*` (5) |
| `meter.rs` | 4 | `crate::meter::*` (11) |
| `morph.rs` | 2 | `crate::morph::*` (6) |
| `print_curve.rs` | 2 | `crate::print_curve::*` (7) |
| `process.rs` | 4 | `crate::process::*` (4) |
| `register.rs` | 2 | `crate::register::*` (3) |
| `stats.rs` | 1 | `crate::stats::*` (9) |
| `toning.rs` | 1 | `crate::toning::*` (3) |
| `transfer.rs` | 1 | `crate::transfer::*` (2) |
| `phase.rs` | 1 | `crate::phase::*` (3) |
| `rgbscan.rs` | 1 | `crate::rgbscan::channel_means` (1) |
| `pool.rs` | 2 | thread-pool knobs (see 3c) |

Pure-only modules with **no** wrapper (use directly): `detect`, `distance`,
`lsd`, `sift`, `median`, `inpaint`, `seam`, `orient`, `shapes`, `memo`, `png`,
`mathf`, `exptab`.

### 3b. Concrete signatures `negpy_egui` will call (already `pub`)

```rust
pub fn percentiles(values: &[f32], qs: &[f64]) -> Option<Vec<f32>>;              // stats
pub fn np_median(values: &mut [f32]) -> f32;                                     // stats
pub fn luminance(rgb: &[f32], dst: &mut [f32]);                                  // analysis
pub fn to_log_density(img: &[f32], dst: &mut [f32]);                             // analysis
pub fn rgb_to_lab(src: &[f32], dst: &mut [f32], s: LabSpace);                    // color
pub fn to_uint<T: Level>(src: &[f32], dst: &mut [T]);                            // convert
pub fn density_histogram(s: Samples) -> Vec<f64>;                                // histogram
pub fn waveform(s: Samples, kind: Waveform) -> Vec<f64>;                         // histogram
// Public config/view structs already exportable:
pub struct Piecewise;  pub struct LabSpace;  pub struct PrintCurve<'a>;
pub struct Lut<'a>;    pub struct Lut3<'a>;  pub struct Transfer;  pub struct Toning;
pub struct Carrier<'a>; pub struct Lit;      pub struct FilmBase;  pub struct BandParams;
pub enum   Waveform;
```

**Action items for Layer 1 (PyO3-visible signatures untouched):**

1. **Visibility audit** — pure `pub fn`s reachable from a wrapper are already
   `pub`. Promote any private `fn` called only from `py/*` to `pub` (additive).
2. **View types** (`histogram::Samples`, `zones::Frame`) must be `pub`.
3. **Do NOT touch `#[pyfunction]`/`#[pyclass]`/`#[pymethods]`.**
4. **No feature flag needed**: `negpy_egui` depends on `negpy_core` with
   `default-features = false`, so pyo3/numpy never compile into the GUI.

### 3c. Thread-pool control (`src/py/mod.rs`, `src/py/pool.rs`)

`detach()`, `BACKGROUND`, `all_cores()`, `background()`, `large_pool()` encode
GIL-free rayon pooling. `negpy_egui` has no GIL: it owns its own `rayon` pool
sized by `available_parallelism()`, leaving `negpy_core` untouched.

---

## 4. `negpy_egui` Workspace & Crate Split

### 4a. Crate names (decided)

`negpy-app` → **`negpy-backend`** (rename). `-app` and `-desktop` were too
similar; this name states the role. Final crate set:

| Crate | Package name | Lib target | Role |
|---|---|---|---|
| backend | `negpy-backend` | `negpy_backend` | all headless logic; no Qt/egui |
| gui | `negpy-desktop` | `negpy_desktop` | `egui`/`eframe` frontend + app shell |
| core (reused) | `negpy-core` | `negpy_core` | path dep `../negpy_core`, `default-features = false` |

> Optional third name if you want to be explicit later: split the GUI shell out of
> `negpy-desktop` — but **not needed now** (see 4d).

### 4b. Workspace decision

- **`negpy_egui/` is its own Cargo workspace.** No root `Cargo.toml` at the repo
  root. `negpy_core/` stays untouched: own workspace, maturin target, own lockfile.
- `negpy_core` is consumed by `negpy_egui` as a **path dependency**, **not** a
  workspace member — so `negpy_core/Cargo.lock`, `flake.nix`
  (`importCargoLock { lockFile = ./negpy_core/Cargo.lock }`) and
  `[tool.uv].cache-keys` need **zero** edits.

### 4c. Tree

```
NegPy/                                 (repo root — NO Cargo.toml added)
├── pyproject.toml                     (unchanged)
├── negpy/                             (unchanged Python package)
├── negpy_core/                        (unchanged: own workspace, maturin target)
└── negpy_egui/                        (NEW — its own Cargo workspace)
    ├── Cargo.toml                     [workspace] members = ["backend", "desktop"]
    ├── rust-toolchain.toml            (1.99, mirrors negpy_core)
    ├── backend/                       crate `negpy-backend`
    │   ├── Cargo.toml                 negpy_core = { path = "../../negpy_core", default-features = false }
    │   └── src/
    │       ├── lib.rs
    │       ├── kernel/                == negpy/kernel/
    │       ├── domain/                == negpy/domain/
    │       ├── features/              == negpy/features/
    │       ├── infrastructure/        == negpy/infrastructure/
    │       ├── services/              == negpy/services/
    │       └── session/               == negpy/desktop/{session,render_memo,...}  (headless parts)
    └── desktop/                       crate `negpy-desktop`
        ├── Cargo.toml                 negpy-backend = { path = "../backend" }
        └── src/                       == negpy/desktop/
            ├── controller.rs  main.rs  app.rs
            ├── view/{canvas,widgets,sidebar,styles}/  *.rs
            └── workers/
```

### 4d. The `negpy-desktop` ↔ `negpy-backend` seam (grounded in the code)

The Python `negpy/desktop` package mixes three concerns. Verified sizes/facts:

| Python file | Lines | Qt? | Belongs to |
|---|---|---|---|
| `controller.py` | 10,471 | **yes** — 627 Qt refs, **162** `pyqtSignal`s | **split** (state → backend, slots → GUI) |
| `session.py` | 2,876 | **yes** — `AppState`, `AssetListModel(QAbstractListModel)`, `DesktopSessionManager(QObject)` | **split** |
| `main.py` | 336 | yes (`MainWindow` bootstrap) | **GUI** |
| `converters.py` | 51 | yes (`QImage`) | **GUI** |
| `file_manager.py` | 114 | yes (trash via Qt) | **GUI** |
| `render_memo.py` | 88 | no | **backend** |
| `prefetch_logic.py` | 22 | no | **backend** |
| `auto_sliders.py` | 69 | no | **backend** |
| `settings_catalog.py` | 755 | no | **backend** |
| `sticky.py` | 186 | no | **backend** |
| `power_assertion.py` | 200 | no | **backend** |
| `startup.py` | 60 | no | **backend** |
| `windows_data_dialog.py` | 184 | no | **backend** |
| all 13 `workers/*.py` | — | **yes** (4–44 Qt refs each) | **GUI** |
| all `services/rendering/*` | — | **no** | already backend |

**Rule for the seam:**

- **`negpy-backend` owns all logic and state.** It has **no** dependency on
  `negpy-desktop`. Its `session/` module holds the pure state machine
  (`WorkspaceConfig`, `AppState`, selection/edit state, `RenderParams`, history)
  and a **render service** that runs the pipeline on a background thread, exactly
  as `services/rendering/*` already does today with zero Qt.
- **`negpy-desktop` owns only presentation + app shell.** It depends on
  `negpy-backend` (one-way). Controller *slots*, `QObject`/`pyqtSignal` plumbing,
  `QImage` conversion, all `view/`, and all `workers/` live here.
- **The callback direction is one-way:** the GUI subscribes to backend events.
  Port `pyqtSignal` → a `crossbeam-channel` of typed `BackendEvent`s (or a
  `tokio`-free `std::sync::mpsc` with a `egui::Context` repaint wake). The GUI
  pushes typed *commands* (`SetParam`, `RequestRender`, `LoadFrame`) into the
  backend; the backend pushes `RenderReady`, `MetricsReady`, `ScanProgress`,
  `BatchProgress` back. **No `AppController` equivalent calls into the GUI.**
- **What splits out of `controller.py`:** the 162 controller signals are the exact inventory
  of the event contract. Group them:
  - *state/data signals* → backend events: `image_updated`, `preview_loaded`,
    `metrics_available`, `roll_folder_missing`, `thumbnail_*`, `export_finished`,
    `pick_better_ready`.
  - *pure UI signals* → stay GUI-local (`crop_guide_changed`,
    `clipping_overlay_changed`, `zones_style_changed`, `cursor_probe_changed`, …).
  - *request signals* → GUI→backend commands: `render_requested`,
    `preview_load_requested`, `normalization_requested`, `batch_autocrop_requested`,
    `strip_requested`, `thumbnails_requested`.
- **The 5 existing back-edges** (`controller`/`session` → `view`) are the only
  ones to unwind, and all 5 are cosmetic imports, not logic:
  `view.canvas.toolbar.CANVAS_COLORS`, `view.widgets.half_frame_dialog.HalfFrameDialog`,
  `view.styles.templates.box_toggle`, `view.canvas.hud.INFO_LEVEL_LABELS`,
  `view.confirm.confirm_*`, `view.widgets.contact_sheet_dialog.ContactSheetDialog`,
  and `session → view.canvas.crop_guides.CropGuide`. In Rust: move `CANVAS_COLORS`
  and `INFO_LEVEL_LABELS` (plain constants) to backend; the rest become
  GUI-side callbacks/trait objects the backend invokes indirectly or drops.

### 4e. Why not one crate per `negpy/<pkg>`?

Section 0a: the five backend packages are one SCC. Five separate backend crates
require breaking those back-edges in *source* — a Python-behaviour change,
forbidden by the "zero breaking changes" rule. Deferred to an optional Phase 2;
the folder seams above make it a mechanical `git mv` once the Rust back-edges are
gone.

### 4f. Why `maturin`/`pyproject.toml`/`flake.nix` are unaffected

- maturin builds **`negpy_core`** only: `pyproject.toml` stays in `negpy_core/`.
  No root manifest exists to change resolution.
- `negpy_core/Cargo.lock` remains authoritative (still its own workspace).
- `negpy_egui/` has no `pyproject.toml`; never seen by `uv`/`pip`/PyInstaller.
- `cd negpy_core && cargo …` (Makefile) still works unchanged.

### 4g. Verification loop (per AGENTS)

```sh
cd negpy_egui && cargo check --workspace && cargo test --workspace
cargo check -p negpy-backend                   # asserts no pyo3 in its graph
cargo check -p negpy-backend                   # asserts no egui/eframe in backend too
cd ../negpy_core && cargo check                # Python build still builds
uv run maturin build --manifest-path negpy_core/Cargo.toml --features python   # parity
make rust                                      # existing fmt/clippy/test for negpy_core
```

---

## 5. Module → Crate Ownership Map

| `negpy/…` (Python) | `negpy_egui/…` (Rust) | Crate |
|---|---|---|
| `negpy/kernel/` | `backend/src/kernel/` | `negpy-backend` |
| `negpy/domain/` | `backend/src/domain/` | `negpy-backend` |
| `negpy/features/` | `backend/src/features/` | `negpy-backend` |
| `negpy/infrastructure/` | `backend/src/infrastructure/` | `negpy-backend` |
| `negpy/services/` | `backend/src/services/` | `negpy-backend` |
| `negpy/desktop/{render_memo,prefetch_logic,auto_sliders,settings_catalog,sticky,power_assertion,startup,windows_data_dialog}` | `backend/src/session/…` | `negpy-backend` |
| `negpy/desktop/{controller,session}` **state half** | `backend/src/session/…` | `negpy-backend` |
| `negpy/desktop/` **rest** (`main`, `view/`, `workers/`, controller slots) | `desktop/src/…` | `negpy-desktop` |
| `negpy/translations/` | `backend/src/assets/translations/` | `negpy-backend` (`include_str!`) |
| `negpy/__init__.py`, `VERSION` | `backend/src/version.rs` | `negpy-backend` |
| `negpy_core/` (PyO3 ext) | path dep `../negpy_core` | `negpy-core` |

### 5a. NegFe asset and data boundary

NegFe is the Rust app. It keeps its own copied assets and its own user data, so a
NegPy install and a NegFe install can run side by side.

| Kind | NegPy (read-only source) | NegFe (owned copy) |
|---|---|---|
| Name | NegPy | NegFe |
| Rust workspace | `negpy_egui/` | `negfe/` |
| Bundled catalogs | `gear/`, `icc/`, `crosstalk/` | `negfe/assets/{gear,icc,crosstalk}/` |
| Translations | `negpy/translations/*.json` | `negfe/assets/translations/*.json` |
| User data | `~/Dokumenty/NegPy/` | `~/Dokumenty/NegFe/` |
| Databases | `edits.db`, `settings.db` | same schema, new directory |

The copy is opportunistic: move one asset family when its Rust code first reads
it, not in one sweep. Until a family is copied, the Rust code may read the NegPy
file through a single `assets_root()` helper. The helper is the only place that
knows both trees, so the final switch is one edit.

`negpy_egui/AGENTS.md` still names the workspace. Renaming the directory to
`negfe/` is a separate change that updates the path dependency, the Makefile
target, the dependency-graph script and this plan in one commit.

Rust naming: packages `negpy-backend` / `negpy-desktop`; lib targets
`negpy_backend` / `negpy_desktop`.

---

## 6. Summary

**Crate naming:** `negpy-app` → **`negpy-backend`**; GUI crate is
`negpy-desktop`. Core reused as `negpy-core`.

**Per-package crate mapping:** `negpy/desktop → negpy-desktop` works, but
`negpy/{kernel,domain,features,infrastructure,services}` → separate crates does
**not** (one dependency cycle). They live as 1:1-mirrored **modules** inside
`negpy-backend`; the split is deferred to an optional Phase 2.

**The seam:** `negpy-backend` owns all logic + state + the render service (no Qt
/egui, one-way dependency); `negpy-desktop` owns presentation, `QObject`→channel
event wiring, and the app shell. `controller.py`'s 162 signals split into backend
events, GUI-local UI signals, and GUI→backend commands; its 5 `controller/session
→ view` imports are cosmetic and move to backend as plain constants or GUI
callbacks.

**Workspace:** `negpy_egui/` is its own workspace
(`members = ["backend", "desktop"]`); `negpy_core` is a path dependency, not a
member, so its lockfile, `flake.nix`, and `[tool.uv].cache-keys` are untouched.

**Bottom-up port order:** `domain/kernel leaves` → `models & serialization` →
`loaders` → `feature kernels` → `rendering engine` → `export/scan services` →
`assets & headless desktop glue` → `egui widgets/canvas/sidebar` → `controller/main`.

**Third-party → crate quick map:** `rawpy→rawler`, `numpy→ndarray`,
`tifffile→tiff`, `imagecodecs→jxl-oxide/zune-jpeg/png`,
`imageio`+`PIL→image`+`imageproc`(+`qcms`), `jinja2→minijinja`,
`PyQt6→egui`/`eframe`, `pyqt6-charts→egui_plot`, `qtawesome→egui-phosphor`,
`piexif→kamadak-exif`+`img-parts`, `onnxruntime→ort`/`tract`,
`wgpu(PyPI)→wgpu`, `pyserial→serialport`, `pyopticfilm/pieusb/usb→rusb`/`nusb`,
`nkscan→nkscan`, `sane→sane`, `gphoto2→gphoto2`, `rendercanvas→egui_wgpu`;
stdlib `sqlite3→rusqlite`, `json/tomllib→serde_json/toml`,
`hashlib→sha2/blake3`, `urllib→ureq`, `ctypes→windows-sys/libc`.

**`negpy_core` decoupling:** pure `pub fn` kernels already exist behind every
`py/*` wrapper — work is a visibility audit + additive `pub` promotions only,
with **zero** `#[pyfunction]`/`#[pyclass]` signature changes.

---

## 7. Implementor Handoff — Task Backlog

Scope: stand up the `negpy_egui/` workspace and land the backend, in the strict
bottom-up order of Section 2. GUI work is now **allowed**: start with E0 (the
Rust capture harness against the golden set), then E1. A panel still waits for
the Layer 2 service it displays, but GUI shell, theme, widgets and capture tests
may land before every backend service is complete.

### 7.0 Definition of Done (applies to every task)

- `cargo check --workspace && cargo test --workspace` green from `negpy_egui/`.
- `cargo check -p negpy-backend` shows **no** `pyo3`, `numpy`, `egui`, or `eframe`
  in its dependency graph (`cargo tree -p negpy-backend` inspected).
- Python side untouched: `cd negpy_core && cargo check` and the maturin build still
  pass; `git status` shows no changes under `negpy/` or `negpy_core/`.
- New logic has Rust unit tests; where a Python module is the oracle, port its
  `tests/test_<module>.py` assertions as Rust tests.
- No file is ported that a lower layer hasn't been; no stubs left behind a ported
  widget.

### 7.1 Phase A — Workspace scaffolding (do first, unblocks everything)

| # | Task | Deliverable / verify |
|---|---|---|
| A1 | Create `negpy_egui/` with `Cargo.toml` `[workspace] members = ["backend","desktop"]`, `resolver="3"`, `rust-toolchain.toml` (1.99). | `cargo check --workspace` on empty crates passes. |
| A2 | `backend/` crate `negpy-backend` → `negpy_core = { path="../../negpy_core", default-features=false }`. | `cargo tree -p negpy-backend` has no pyo3/numpy. |
| A3 | `desktop/` crate `negpy-desktop` → `negpy-backend = { path="../backend" }` + `eframe`/`egui`/`egui_extras`/`egui_wgpu`/`wgpu`. | `cargo tree -p negpy-desktop` has no pyo3. |
| A4 | Add CI step mirroring Section 4g; add `Makefile` target `rust-egui`. | CI green on the empty skeleton. |
| A5 | Confirm `negpy_core/Cargo.lock`, `flake.nix`, `[tool.uv].cache-keys` are untouched. | `git diff --stat` shows none of them. |

### 7.2 Phase B — `negpy_core` decoupling (Layer 1)

| # | Task | Deliverable / verify |
|---|---|---|
| B1 | Run the visibility audit (Section 3 action 1): every pure `pub fn` a `py/*` wrapper calls is `pub`. Promote stragglers. | No `#[pyfunction]`/`#[pyclass]` signature changed (`git diff` on `py/` is empty). |
| B2 | Confirm `histogram::Samples`, `zones::Frame` and the `pub struct/enum` config types are `pub`. | `cargo check -p negpy-core --no-default-features` builds. |
| B3 | Smoke test: `negpy-backend` calls 3 kernels directly (`stats::percentiles`, `color::rgb_to_lab`, `convert::to_uint`) on `&[f32]`. | Unit test passes; no Python involved. |

### 7.3 Phase C — Event contract (do before `controller`/`session` port)

**Task C1 — define the typed event/command enums.** This is the single highest-risk
step and it gates the `controller.py` split. The inventory is exact:

- `controller.py` declares **162** `pyqtSignal`s (10,471 lines).
- `session.py` declares **17**.
- `workers/`: `render` **36**, `capture_worker` **28**, `scan_worker` **17**,
  `contact_sheet` **7**, `export` **6**, `stitch`/`library`/`hdr`/`embedding`/`chart_fit`
  **4** each, `triplet_probe`/`frame_merge` **2**, `export_view` **1**.
- `main.py` **1**.
- **Total 456** signals across `negpy/desktop/`.

Sort every one into exactly one bucket and record it in a table in the PR:

1. **Backend event** (backend → GUI): data/state changes the GUI displays —
   e.g. `image_updated`, `preview_loaded`, `metrics_available`,
   `finished`/`metrics_updated` (render), `export_finished`, `library_cleared`,
   `scenes_detected`, `roll_folder_missing`, `pick_better_ready`, scan/capture
   progress+finished+error families.
2. **Backend command** (GUI → backend): requests into the backend —
   e.g. `render_requested`, `preview_load_requested`, `normalization_requested`,
   `batch_autocrop_requested`, `strip_requested`, `scan_*_requested`,
   `capture_requested`, `poll_*_requested`.
3. **GUI-local UI signal** (stays in `negpy-desktop`): purely presentational —
   e.g. `crop_guide_changed`, `clipping_overlay_changed`, `zones_style_changed`,
   `cursor_probe_changed`, `tool_sync_requested`, `negative_peek_changed`.

| # | Task | Deliverable / verify |
|---|---|---|
| C1 | Enums `BackendEvent`, `BackendCommand` in `negpy-backend`; the 456-signal → bucket table committed. | Every signal classified; no signal in two buckets. |
| C2 | Channels: `crossbeam_channel` (commands in, events out) with an `egui::Context::request_repaint()` wake on the event side (AGENTS "Repaint on Demand"). | A round-trip test: send command → receive event → repaint requested. |
| C3 | Backend render service runs on a dedicated worker thread; latest-wins coalescing on `RenderParams` (AGENTS rules 2–3); proxy ≤1600px while dragging, full-res on drag-stop / 150ms debounce. | Unit test: rapid `RenderParams` pushes → only newest processed. |
| C4 | Document the mapping from the Python `AppController`/worker classes to this contract. | One page; lists each signal's home. |

> Do C1–C4 **before** porting `controller.py` so the backend state machine can be
> written against a frozen contract and the GUI/backend diffs don't tangle.

### 7.4 Phase D — Layer 2 services (one task per module group)

Follow Section 2's ordered checklist. Each row = one PR with tests.

| # | Task | Depends on | Verify |
|---|---|---|---|
| D1 | `kernel.system.{text,i18n,logging,paths,override}` + `assets/translations` via `include_str!`. | A1 | i18n/plural tests from `tests/test_text_*.py`. |
| D2 | `domain.{types,models,tokens,migrations}` (serde DTOs). | D1 | Round-trip JSON parity vs Python fixtures. |
| D3 | `kernel.system.config` (TOML), `kernel/system/override`. | D2 | `tests/test_sync_settings.py`/`test_shipped_defaults.py` ported. |
| D4 | `infrastructure.storage.{repository,local_asset_store}` via `rusqlite` (bundled). | D2 | Schema byte-identical; `tests/test_storage_repo.py` ported. |
| D5 | `services.assets.{sidecar,presets,rolls,toml_cache}` — exact JSON/TOML schemas. | D3,D4 | `tests/test_sidecar.py`, `test_settings_catalog_presets.py`. |
| D6 | Loaders: `infrastructure.loaders.*` (`helpers`, `memory`, `tiff_loader`, `jpeg_loader`, `jxl_loader`, `nef_loader`, `fff_loader`, `pakon_loader`, `noritsu_loader`, `rawpy_loader`, `source_encoding`, `source_kind`, `factory`). | D5 | `tests/test_tiff_loader_encoding.py`, `test_tiff_loader_ir_validity.py`, `test_unsupported_raw_reason.py`. |
| D7 | Feature kernels: `features.process.*`, `exposure.*` (incl. `print_lut`, `normalization`, `papers`, `bw_filter`, `densitometer`). | D3 | `tests/test_source_encoding.py`, `test_exposure_*`, `test_tone_*.py`. |
| D8 | Feature kernels: `geometry.*`, `lab.*`, `local.logic`, `retouch.{logic,clone}`, `toning.*`, `finish.*`. | D7 | `tests/test_geometry_*`, `test_toning_*`, `test_retouch_*`. |
| D9 | Feature kernels: `flatfield.logic`, `hdr.logic`, `lens.*`, `rgbscan.logic`, `stitch.logic`, `transparency.*`, `altprocess.*`, `cyanotype/lith/sabattier`. | D8 | `tests/test_flatfield_*`, `test_hdr_*`, `test_stitch.py`, `test_slide_*`. |
| D10 | Rendering engine: `services.rendering.*` + `services.maps` + `infrastructure.display.{color_mgmt,color_spaces,monitor_profile}` + `gpu.*` (wgpu, 33 WGSL shaders). | D9 | `tests/test_preview_*`, `test_soft_proof_lut.py`, `test_working_oetf.py`. |
| D11 | Export/scan: `services.export.*`, `services.scanning.*`, `services.film_edge.*` (incl. `minijinja` templating). | D10 | `tests/test_templating.py`, `test_contact_sheet*`, `test_*export*`. |
| D12 | Assets: `services.assets.*` (thumbnails, embeddings, semantic_model, migrations/*), `services.capture.*`, `services.view.*`, `domain.session`. | D11 | `tests/test_thumbnail_*`, `test_migrations_*`, `test_session_manager.py`. |
| D13 | Headless desktop glue → `backend/src/session/`: `desktop.{render_memo,prefetch_logic,auto_sliders,settings_catalog,sticky,power_assertion,startup,windows_data_dialog}`; controller/session **state half**. | C1–C4, D12 | `tests/test_render_memo*`, `test_prefetch*`, `test_auto_sliders*`, `test_sticky_*`. |

### 7.5 Phase E — Layer 3 UI (after its backend dependency lands)

Port bottom-up from Section 2's checklist; each row depends on the listed Phase D task.

**GUI parity target.** NegFe keeps the Python app's UX, not its pixels. The
target is the same controls, order, grouping, hierarchy and density; pixel
identity is not the goal. `negpy_egui/tests/gui/compare.py` is a diagnostic:
a score drop over ~0.03 in one capture means a region moved or vanished. The
layout score is not a merge gate, and a task whose only gain is a closer score
is deferred. `DEV_LEDGER.md` lists the deferred polish.

**Golden set (binding).** `negpy_egui/tests/gui/golden/tall/` is the reference
set. It holds 19 window captures at 1920x1600 and one Contact Sheet dialog at
1080x760, with a manifest that names the tab, sub-tab and view of each:

| Golden | State |
|---|---|
| `01_library` | Library tab, roll list |
| `02_roll_top` | Roll tab, Setup at the top |
| `03_roll_process` | Roll tab, process card in view |
| `04_frame_geometry` | Frame, Geometry |
| `05_frame_tone` | Frame, Exposure |
| `06_frame_color` | Frame, Look |
| `07_frame_finish` | Frame, Finish |
| `08_frame_favorites` | Frame, Favourites (`favourites`) |
| `09_frame_history` | Frame, History |
| `10`–`13_metadata_*` | Shot, Process, Presets, My Gear |
| `14`–`17_export_*` | File, Proof, Presets, Alongside Exports |
| `18_scan` | Scan |
| `19_light_table` | Light Table, the whole roll in rows |
| `20_contact_sheet_dialog` | Contact Sheet dialog with rendered tiles |

A GUI change is compared against the matching golden as a smoke test before it
merges. A new golden is added only with its `manifest.json` entry.

**E0 — Rust capture and comparison harness (first GUI task, before E1).**

The Python harness already exists. Port the same procedure to the Rust app:

- `desktop/src/capture.rs` takes the screenshot with
  `ctx.send_viewport_cmd(ViewportCommand::Screenshot(egui::UserData::default()))`
  and reads `egui::Event::Screenshot { image, .. }` from the next frame's input.
  `image` is an `Arc<epaint::ColorImage>` with `size: [usize; 2]` and
  `pixels: Vec<Color32>`; write those pixels as PNG.
  The capture uses a fixed 1920x1600 inner size, no decorations, and scale 1.
- `desktop/src/fixture.rs` builds the same state as `capture_matrix.py`: the
  copied profile, `tutorial_seen`, the camera/white-light rig, `P5080635.ORF`,
  the Roll tab, the Light Table and the Contact Sheet dialog. No modal restore
  prompt and no file picker.
- `desktop/src/bin/negfe-capture.rs` is the test binary. It writes one PNG per
  manifest entry and a `manifest.json` with the same fields as the Python one.
- A capture is accepted only when the image size equals the manifest. A missing
  frame, an open dialog or a blank canvas fails the capture before comparison.
- The comparator stays `tests/gui/compare.py`; the Rust binary only produces
  the candidate PNGs. One comparator keeps both apps on one score.
- Extract the layout from the Python code, not from the screenshot alone:
  `theme.py` for tokens, `templates.py` for the control factories,
  `main_window.py` for the shell and splitters, `right_panel.py` for tabs and
  sub-tabs, and `controls_panel.py` for card order and card membership. The
  screenshot shows the result; the code says why.

Extracted layout facts for E1–E7:

| Fact | Source value |
|---|---|
| Window margins | `THEME.space_lg` = 8 px on every side |
| Type sizes | small 12, base 13, header 14, title 16, micro 9 |
| Spacing scale | 2, 4, 6, 8, 12, 16 px |
| Radius scale | 3, 4, 6 px |
| Icon button width | `ICON_BUTTON_WIDTH` = 36 px |
| Field label width | `FIELD_LABEL_WIDTH` = 90 px |
| Section header | `HEADER_HEIGHT` = 40 px, button 24, icon 13 |
| Toolbar sections | `TOOLBAR_BUTTON_HEIGHT` = 28 px, icon 16 |
| Canvas bar | `BAR_BUTTON_HEIGHT` = 36 px, icon 18, track button 33 |
| Top row | `TOP_ROW_HEIGHT` = 44 px |
| Right tab strip | 58 px; Frame sub-tab strip 38 px |
| Analysis split | `min(320, height * 3 // 10)`; 320 px at 1600 |
| Shell at 1920x1600 | canvas 1384x1236 at (8, 52); rail 514x1540 at (1398, 52); strip 248 px; toolbar 52 px |
| Card order | `controls_panel.pages`; `RightPanel._build_roll_page` for Roll |
| Tab order | Library, Roll, Frame, Metadata, Export, Scan; Frame sub-tabs Geometry, Exposure, Look, Finish, Favorites, History |

Verification: `cargo run -p negpy-desktop --bin negfe-capture -- --golden tests/gui/golden/tall`
writes `tests/gui/actual/rust/`; then `python3 tests/gui/compare.py` scores each
pair. The first Rust capture may fail while the UI is incomplete. It must still
produce every PNG and a manifest.

| # | Task | Depends on |
|---|---|---|
| E0 | Rust capture binary, fixture state, and golden comparison run. | A3 |
| E1 | `view.styles.*` (theme, fonts, color_vision, templates) + `egui-phosphor` icons. | A3, E0 |
| E2 | `view.widgets.*` primitives (sliders, segmented_choice, fold, collapsible, split_button, step_combo, tab_header, …, `egui_plot` charts). | E1 |
| E3 | `view.canvas.*` + widgets (crop guides, navigator, hud, zone legend, overlay, toolbar) via `egui::Painter` + `egui_wgpu`. | E2, D10 |
| E4 | Shortcuts: `view.{shortcut_registry,shortcut_editor_search,shortcut_hints,keyboard_doc,keyboard_shortcuts,window_menu}`, `slider_targets`, `slider_shortcut_groups` (spec: `docs/KEYBOARD.md`). | E2 |
| E5 | `view.sidebar.*` panels, each over its D-task service (tone, color, geometry, lab, local, retouch, process, finish, toning, flatfield, hdr, lens, demosaic, trichrome, sensor, scan*, export, presets, metadata, history, library_tree). | E3, E4, D7–D12 |
| E6 | Dialogs: `view.widgets.*` dialogs (preferences, export_presets, roll_settings, gear_catalog, sensor_calibration, contact_sheet, command_palette, chart_calibration, tutorial_*, update_dialog). | E5 |
| E7 | Shell: `view.{file_actions,frame_merge_action,sidecar_action,main_window,mac_window,mac_menu_bar,update_notice,confirm}` + `desktop/{controller,main,session,workers}` wired to the C1 contract. | E6, D13 |
| E8 | Packaging: run the GUI binary against a real RAW folder; confirm preset/sidecar/export parity with the Python app on the same inputs. | E7 |

### 7.6 Risk register

| Risk | Where | Mitigation |
|---|---|---|
| `controller.py` split tangles state and slots | Phase C/E7 | Freeze the event contract (C1) **before** porting; port state half in D13 with no GUI imports. |
| `rawpy` demosaic parity | D6 | `RawDecoder` trait; golden-image tests from `tests/` + `icc/`. |
| Lockfile/flake churn | A5 | Not applicable by design — `negpy_core` stays its own workspace; assert with `git diff --stat`. |
| JSON/TOML byte parity | D2,D3,D5 | Fixture round-trip tests against real `.negpy`/preset files. |
| Backend accidentally pulling GUI deps | all | `cargo tree -p negpy-backend` gate in CI (Section 4g). |

### 7.7 Suggested first PR

Phase A (A1–A5) only: empty `backend/` + `desktop/` crates, workspace wiring,
CI gate, `cargo tree` assertions. Land it before any port so every later PR has a
green skeleton to build on.

---

## 8. Implementor Compliance (negpy_egui/AGENTS.md — mandatory)

This section restates the binding rules from **`negpy_egui/AGENTS.md`** (the sole
source of truth — see the SOURCE OF TRUTH section) and applies them to the Phase
A–E backlog. **Where this section, a task description, or any other part of this
plan disagrees, `negpy_egui/AGENTS.md` wins.**

### 8.1 Commit style (AGENTS.md "Commit Style & Rules")

- **Atomic:** one logical change per commit. If the message needs "and", split the
  commit. (Consequence for Phase D: D7 is *not* one commit — it is at least four
  separate feature packages.)
- **Size:** target **200–400** changed lines; hard cap **1,000** for pure additions
  (tests, assets, boilerplate). A ported module that exceeds 1,000 lines is split by
  function group.
- **Green builds:** every commit compiles and passes all existing tests **before**
  the next task. Per `AGENTS.md`, tests run **only for `negpy_egui/`** — all
  development happens there (`cd negpy_egui && cargo test --workspace`). The
  `negpy/` and `negpy_core/` trees must stay untouched; a commit that edits them
  must be justified explicitly (additive `pub` promotions in `negpy_core/src/*.rs`
  only — see 8.5).
- **Test cadence:** run targeted tests for each commit. After the tenth commit,
  run the source audit in §8.7, then `cargo check --workspace` and
  `cargo test --workspace`. A new session may trust the previous session's
  workspace result; record that result and its time in `DEV_LEDGER.md`.
- **Format (Linux Kernel style):**
  - Subject ≤ **50** chars: `<scope>: <imperative summary>`, no trailing period.
  - One blank line between subject and body.
  - Body wrapped strictly at **72** chars; explain **what** then **why**. Omit the
    body for trivial 1–5 line fixes.
  - Tone: concise/technical. Banned filler: "Update", "Fix bug", "Refactor code",
    "This commit aims to…".

**Scopes for this project** (pick the crate the change lands in):

| Scope | Use for |
|---|---|
| `negpy_egui` | workspace scaffolding, workspace-level config |
| `backend` | anything in crate `negpy-backend` |
| `desktop` | anything in crate `negpy-desktop` |
| `core` | additive `pub` promotion in `negpy_core/src/*.rs` |
| `plan` | edits to `MIGRATION_PLAN.md` / `DEV_LEDGER.md` |

**Worked examples for this migration:**

```text
negpy_egui: scaffold backend and desktop crates
```
```text
backend: add domain config DTOs with serde parity

Mirror WorkspaceConfig and the per-feature config dataclasses so
sidecars and presets round-trip byte-for-byte with the Python
serializer.

Add round-trip tests against captured .negpy fixtures.
```
```text
core: promote stats percentile helpers to pub
```

### 8.2 10-commit batch stop (AGENTS.md "Workflow Checkpoints & Development Ledger")

- **Stop after 10 commits in one session.** Do not start an 11th. Return control to
  the user.
- **Post-batch order:** after commit #10, run the §8.7 source audit, write its
  findings into §9.9 and `DEV_LEDGER.md`, run the workspace check, then emit the
  halt line. The audit does not authorize an 11th commit.
- The Phase A–E backlog is ~33 tasks; at 10 commits/session that is **≈4 sessions**.
  Sequencing across sessions is carried by `DEV_LEDGER.md`, not chat history.
- A batch takes up to ten unchecked **Next Steps** from `DEV_LEDGER.md`, in order.
  Stop earlier only at a blocker or an explicit checkpoint.
- **Halt output (verbatim, nothing else):**
  `Checkpoint reached (10 commits). DEV_LEDGER.md updated. Ready for context reset.`

This is a local override requested on 2026-11-21. `negpy_egui/AGENTS.md` still
states a 5-commit stop and a 60-line ledger until a human reconciles it.

### 8.3 Ledger keeping (AGENTS.md "Workflow Checkpoints & Development Ledger")

- `DEV_LEDGER.md` lives at the **repo root** and is updated **before every commit**
  (and always right before halting at commit #10).
- Keep it **under 120 lines**; overwrite completed items instead of growing it.
- It must be sufficient for a fresh session with **zero chat history** to resume:
  current goal, recent completed commits, ordered next steps, and gotchas.
- A seed ledger is committed at `DEV_LEDGER.md`; the implementor keeps it current.
  Do **not** let the ledger drift from the Phase A–E task IDs — reference tasks as
  `A1`, `D7`, etc.

### 8.4 Session protocol

1. Read `DEV_LEDGER.md` first; resume at the first unchecked **Next Step**.
2. Work the current task; keep each commit atomic and green.
3. Update `DEV_LEDGER.md` **before** each commit.
4. On commit #10, run the §8.7 source audit, write it into §9.9 and
   `DEV_LEDGER.md`, run the workspace check, then emit the §8.2 halt line and
   stop.

### 8.5 Guardrail cross-checks (AGENTS.md "Coding & Workflow Guardrails")

- **Use a crate first:** when a standard format, container or algorithm has a
  maintained crate, depend on it instead of implementing the format. A crate that
  is absent from the local cache is not a reason to hand-roll: run `cargo fetch`
  or `scripts/fetch-crates.sh`, then commit the lockfile change with the port.
- **Anti-Loop Rule:** a compiler/borrow-checker error persisting after **3 attempts**
  ⇒ STOP, explain the exact conflict, ask for human guidance. This is *not* a
  session halt; do not silently retry a 4th time.
- **Zero upstream edits:** `git diff --stat` must show nothing under `negpy/`; only
  additive `pub` promotions under `negpy_core/src/`.
- **Verification Loop:** `cargo check --workspace` + `cargo test --workspace` after
  every module port (run from `negpy_egui/`). On commit #10, run the §8.7 source
  audit before the workspace check.

### 8.6 Workspace state at handoff (verified)

- `negpy_egui/` holds the `backend/` and `desktop/` crates. The backend has the
  Layer 2 services landed through D13; the desktop crate is still a shell.
- **GUI work is allowed.** The binding reference is
  `negpy_egui/tests/gui/golden/tall/`: 19 window captures at 1920x1600 and one
  Contact Sheet dialog at 1080x760, with `manifest.json`.
- E0 is the first GUI task: the Rust capture binary, the fixture state, and the
  first layout score. It depends only on the desktop crate skeleton.
- The Python capture harness and comparator already exist:
  `capture_matrix.py`, `capture_negpy.py`, `compare.py`, and
  `GUI_COMPARE.md`.
- **`negpy_egui/AGENTS.md`, `MIGRATION_PLAN.md`, and `DEV_LEDGER.md` are listed in
  `.git/info/exclude`** (patterns `AGENTS.md`, `MIGRATION_PLAN.md`,
  `DEV_LEDGER.md`). Because the `AGENTS.md` pattern has **no slash**, it matches at
  *any* depth — so `negpy_egui/AGENTS.md` is currently **ignored by git**. If the
  rules file must be tracked (recommended, since it is the source of truth), remove
  that pattern or anchor it (`/AGENTS.md`).
- Before each session, run `git status` and confirm the tree shows only the files
  the current task touches.

### 8.7 Post-batch source audit (requested 2026-11-21)

Run this after commit #10 and before the workspace check. It is a read-only
source audit, not another implementation task.

1. Inspect `negpy_egui/backend/src` for features whose Python source is wired but
   whose Rust entry point is missing, a stub or unused.
2. Inspect `negpy_egui/desktop/src` with the six checks in §9.7: click, field,
   read, state, service and regression test.
3. Compare every user-visible Python feature with the Rust shell, including
   controls that are painted but have no response.
4. Trace the histogram path from the render output to each chart and readout.
5. Record confirmed findings in §9.9 and `DEV_LEDGER.md`. Do not create an 11th
   commit in the same session to fix them.
6. Run `cargo check --workspace` and `cargo test --workspace` from `negpy_egui/`.
   Report a failure instead of starting an 11th commit.

---

## 9. Wired-GUI Gap Register (code audit, 2026-11-19)

This register records what the desktop shell renders but does not yet make work.
It is derived from `negpy_egui/desktop/src`, not from screenshots. A gap means one
of four things:

- **Dead:** painted or allocated without a click response.
- **Local:** a control changes only view state while the file, store or pipeline
  does not read it.
- **Stub:** the surface exists but the named behavior is absent.
- **Missing:** the workflow has no entry point in the Rust shell.

Priority reflects user impact, not implementation cost. Within a priority, fix
the shared cause before the individual call sites.

**Status note (2026-11-21):** the 2026-11-19 tables are the original baseline.
Where they disagree with §9.9, §9.9 is current: crop redraw, Auto Crop,
the output histogram, card collapse state, retouch bakes, canvas retouch
gestures, unlimited local masks, export sidecars and Folder as Roll are now
wired.

### 9.1 P0 — the application shell cannot run a real session

> **Historical.** This table is the 2026-11-19 baseline. §9.9 and the active
> register supersede it: the shell now starts a session, renders a frame and
> shows measured data.

| # | Gap | Evidence | Class |
|---|---|---|---|
| P0-1 | **No profile is resolved or retained at startup.** A launch with no `--profile` or `NEGPY_GUI_PROFILE` runs with `profile = None`, although `default_user_dir()` and the Windows preflight exist in the backend. The shell never calls either, so the Library has no store and every edit is profile-free. The profile is also never saved on the app state, so a later import has no profile even after the default is resolved. | `negfe.rs:24`; `launch.rs:42-51`; `app.rs:191-195`; unused `session/startup.rs` | Missing |
| P0-2 | **Import Roll cannot succeed without a profile.** The picker opens from the Import menu, but `LibraryState::import` returns `false` immediately when `self.profile` is `None`, and the UI has no message for that branch. This is the reported symptom. | `library.rs:188-191`, `262-302`; `negpy_egui/desktop/src/app.rs:190-195` | Local |
| P0-3 | **The Library is never replaced by an empty state.** With no rolls the panel paints the Import toolbar and a `0 rolls · 0 photos` footer, not a “No roll loaded” help state with an import action. A real empty first-run screen is missing. | `library.rs:262-379`; `main.rs`/`negfe.rs` launch path | Missing |
| P0-4 | **Import does not select or open the imported roll or its first frame.** `import` reloads the rows and returns `true`, but the caller discards that result and the row is not opened. The user sees a list change, not a loaded roll and strip. | `library.rs:188-219`, `262-351`; `rail.rs:239-241` | Stub |
| P0-5 | **The manually opened frame is a file name guessed in the fixture, not the selected frame.** Launch looks for `reference.fixture.frame` (`P5080635.ORF`) in the folder; any other roll shows the placeholder and an empty strip. The actual first image in the folder is never chosen. | `launch.rs:84-89`; `frame_source.rs:222-227` | Stub |
| P0-6 | **No frame selection or navigation exists.** Film Strip tiles, Light Table cells and the previous/next controls are painted or hit-tested but do not load a frame, update the edit or save the old one. `FrameEdit` binds one profile and path at load; no app method rebinds it when the frame changes. | `app.rs:217-241`, `459-518` (strip), `586-880` (toolbar), `light_table.rs:71-77`; `edit.rs:20-53`; `launch.rs` loads once | Dead |
| P0-7 | **The canvas never re-renders after an edit.** `set_frame` renders once before the event loop; `draw` only draws the stored texture. A slider change marks the edit dirty and saves it, but the visible photo does not change. There is no render service, worker, cache or texture invalidation in the shell. | `app.rs:217-222`, `260-330`, `339-347`; `frame_source.rs:105-136`; no `RenderService` use in `desktop/src` | Missing |
| P0-8 | **The empty workspace shows a flat beige rectangle that looks like a photo.** It is a fixed `0xa8a0a0` fill; it does not say “No roll loaded”, “Open a folder” or offer the import action. The top bar still prints the fixture file name and recipe. | `app.rs:445-457`, `551-569` | Stub |
| P0-9 | **Analysis is a decorative curve.** The Analysis card paints a fixed `t.powf(0.45)` curve and a grayscale wedge. It never reads the render metrics, frame histogram, density histogram or readouts. The Log pill and the toolbar are decorative. This is the reported histogram failure. | `rail.rs:335-451`; no metrics path in `app.rs` | Dead/Stub |
| P0-10 | **The top bar and toolbar are a screenshot.** Every toolbar control, including Photo/Roll mode, navigation, zoom, rotate/flip, Tools, Undo/Redo, Print, Roll Average and the primary Roll action, is painted into a `Painter` with no `Sense`. | `app.rs:586-880`; `paint_toolbar` takes no `Ui` | Dead |

### 9.2 P1 — card state and shared chrome do not behave as controls

| # | Gap | Evidence | Class |
|---|---|---|---|
| P1-1 | **Roll Analysis cannot stay collapsed.** `show_roll_analysis` creates a new `CollapsibleSection` and sets `expanded = true` on every frame, so a click is undone on the next repaint. | `rail.rs:494-501` | Local |
| P1-2 | **Calibration and every placeholder card do the same.** `show_collapsed` reconstructs the section and forces `Calibration` open or the rest closed every frame. | `rail.rs:503-517` | Local |
| P1-3 | **The Analysis card's `Readouts` sub-section is recreated closed every frame, so it cannot open.** | `rail.rs:444-449` | Local |
| P1-4 | **Metadata cards reset their open state every frame.** `metadata_card` is always called with `expanded = true` for Analog Gear, Capture, Exposure, Process and Scanning, and the returned `*_open` values are discarded. | `metadata.rs:288-462`; `metadata_card` at `571-583` | Local |
| P1-5 | **Scan cards cannot keep a new open state.** `Camera` and `Preset` pass a constant `true`/`false` and discard the returned state; only Device, Quality, Framing and Output persist. | `scan.rs:161-190`; `scan_card` at `223-240` | Local |
| P1-6 | **Crop and Straighten cannot collapse.** Both sections are rebuilt each frame with their default expanded state and the header result is discarded. | `frame_geometry.rs:191-197`, `284-290` | Local |
| P1-7 | **The Analysis header's Log, chart tool buttons and guide icons are painted, not interactive.** | `rail.rs:337-451`; `app.rs:592-600` for toolbar guide | Dead |
| P1-8 | **The main-tab strip's own collapse affordance is absent.** `TabSummary` paints a collapse chevron on the Frame, Metadata and Export pages, but every caller compares the result to `Default::default()` and ignores `Collapse`. | `tab_header.rs:153-199`; callers at `frame_geometry.rs:180`, `metadata.rs:289`, `export.rs:293`, `history.rs:108` | Dead |
| P1-9 | **The Library card is forced open.** | `library.rs:264-266` | Local |

**Shared cause for P1-1..P1-9:** a section's open state belongs in the page or
rail state, like `MeteringState::open`; it must not be recreated or overwritten
during `show`. Add an interaction test that toggles each section header and
reads the state on the next frame.

### 9.3 P2 — controls that look active but change nothing durable

| # | Gap | Evidence | Class |
|---|---|---|---|
| P2-1 | **Export has no action.** `split_action("Export Roll")` and the footer `split_action` are created and dropped. There is no call to `write_export`, a roll export worker, a progress/cancel path or an error surface. | `rail.rs:207-227`; `scan.rs:194`; `services/export/service.rs` exists but is unused by the shell | Missing |
| P2-1b | **The export render path omits the retouch bake.** `render_export_file` decodes and runs the pipeline, but never calls `bake_luma_repairs`; dust removal and IR repair reach the preview, not the exported pixels. | `services/export/render.rs:35-80`, `137-155`; Python equivalent `services/rendering/image_processor.py:849-865` | Missing |
| P2-2 | **Contact Sheet is a capture-only painting.** The `20_contact_sheet_dialog` view paints fixed text, fixed toggles and paper tiles; Cancel/Export and every field are not interactive, and it is only reachable through a manifest `view` key. | `app.rs:304-311`; `contact_sheet.rs:19-35`, `138-312` | Stub |
| P2-3 | **Scan is entirely a stub.** The Scan button, Folder, Refresh, Prescan, Scan as Roll, Folder as Roll, `+`, device discovery/status and write path are absent. | `scan.rs:85-220`; backend `services/scanning` unused | Missing |
| P2-4 | **Library Refresh runs, but its result and the import status go nowhere.** Refresh is behind the More menu; import status is painted as a transient footer string, and the imported/open roll is not propagated to the strip or frame. | `library.rs:261-379`, `559-566`; `rail.rs:239-241` | Local |
| P2-5 | **The Find field is a painted placeholder.** It has `Sense::hover`, a static hint and a static `Ctrl+K`; no text edit, command palette or shortcut dispatch exists. | `rail.rs:683-717`; top-bar placement in `app.rs:416-433` | Dead |
| P2-6 | **The top bar prints fixture facts.** File name, dimensions and the recipe string are hard-coded, not read from the frame or config. | `app.rs:551-569` | Stub |
| P2-7 | **Soft proof is stored but never applied to the preview.** The Proof page writes `ProofSettings` and `app.rs` persists it; `proof_lut`/`gamut_lut` exist in the backend, but the canvas texture is produced before proof and never rebuilt. | `export.rs:268-289`; `app.rs:289-299`; `infrastructure/display/proof_lut.rs` unused by the shell | Missing |

### 9.4 P3 — per-card dead actions and local-only controls

| # | Card / control | Gap | Evidence |
|---|---|---|---|
| P3-1 | Toolbar Photo/Roll, zoom, rotate/flip, Tools, Undo/Redo, Print, Roll Average, Export roll | Painted only | `app.rs:586-880` |
| P3-2 | Frame Strip tiles and scrollbar | Painted; no selection, scrolling or marks | `app.rs:459-518` |
| P3-3 | Analysis Log, chart export/stack/circle, readouts | Painted/dead | `rail.rs:337-450` |
| P3-4 | Roll Analysis Use Frame, Reanalyze, Detect | `let _ = action_with_icon(...)`, no store/pipeline action | `rail.rs:525-541`, `583-592` |
| P3-5 | Metering Measure Alone, Dmin/Dmax, Reset Metering | Measure Alone only toggles a local bool; Dmin/Dmax use `&mut false`; Reset is dropped | `metering.rs:118-148` |
| P3-6 | Raw Decode | No `open` persistence bug here, but no measurement readout; it writes the config only | `raw_decode.rs:105-144` |
| P3-7 | Optics Add/Remove Flat Field | Dropped actions; no file picker or profile removal | `optics.rs:130-146` |
| P3-8 | Calibration Matrix and Strength | Painted combo; Strength is a temporary slider reset each frame | `rail.rs:627-671` |
| P3-9 | Crop Draw/Auto Crop and Straighten rotate/flip | Draw/Auto toggles; rotate/flip are painted chips; Crop All Frames and Square to Film dropped | `frame_geometry.rs:191-282`, `284-350` |
| P3-10 | Tone/Dodge & Burn Clear All | Dropped action; no mask model or canvas interaction | `frame_tone.rs:436-452` |
| P3-11 | Retouch Heal/Scratch/Line/Clone/Match Tone | Toggles persist config, but there is no canvas tool, brush, point model or bake | `frame_finish.rs:281-312` |
| P3-12 | Finishing Drop Empty Corners | Dropped action | `frame_finish.rs:314-336` |
| P3-13 | Metadata Clear, Infer from Folder Name, Description…, lock, Sequence…, Clear (Process/Scanning) | Raw `egui::Button` results are discarded; some controls are not saved because `show_shot` returns only the sub-cards' changes | `metadata.rs:288-462` |
| P3-14 | Export Save Preset…, Alongside Exports | Save is dropped; `alongside` toggles local state but `store` never writes it | `export.rs:292-324`, `store` at `144-177` |
| P3-15 | History Save Work Print…, step rows | Save is dropped; a step click returns `true` without a history/index change (the comment promises navigation/export) | `history.rs:105-152`, `208` |
| P3-16 | Library row open | Row click returns a name, but the Library caller ignores it; no roll is opened | `library.rs:333-336`; `rail.rs:239-241` |
| P3-17 | Frame Favorites | The Favorites page shows a `Presets` placeholder via `show_collapsed`; no preset list, apply or save exists | `rail.rs:322-329`, `747-755`; `frame_cards("favourites")` returns `["Presets"]` |

### 9.5 P3b — field coverage: controls that show or set the wrong field

These are subtler than a dead button: the control paints, stores a value, and the
panel round-trips its own state, but the value never reaches the field the card
claims. Group them with the action work because each needs a field mapping test.

| # | Card | Displayed or stored to | Expected field | Evidence |
|---|---|---|---|---|
| P3b-1 | Filtration | `temperature` slider shows `wb_cyan * 8000 + 4000`, but `store` writes only the three `wb_*` values | A real white-balance temperature, or remove the slider | `frame_tone.rs:211-225`, `265-318` |
| P3b-2 | Filtration | `region` (Global/Shadows/Highlights) never reaches a config field | The region the channel trims apply to, or remove it | `frame_tone.rs:30-44`, `366-381` |
| P3b-3 | Tone | `channel` (Global/R/G/B) never reaches a config field | The per-channel target or remove it | `frame_tone.rs:42-55`, `384-403` |
| P3b-4 | Tone | `paper.paper` selection is never stored | `exposure.paper_profile` | `frame_tone.rs:109-131`, `406-433` |
| P3b-5 | Paper Response | `separation_damping` is stored, but no field maps its rail to Dye Separation; the UI does not encode the neutral rule | `exposure.separation_damping` with the documented gate | `frame_tone.rs:406-433` |
| P3b-6 | Dodge & Burn | The card loads `Burn`/`Grade` from global `shadow_density`/`shadow_grade` and `Flash`/`Feather`/`Tone Zone`/`Tone Softness` from `analysis_buffer`; its edits send those scaled values back to the same global fields. The backend has `LocalAdjustmentsConfig`, but the card never reads or writes a mask. | `LocalAdjustmentsConfig` (`backend/src/domain/models/local.rs`) | `frame_tone.rs:249-260`, `436-452`, `265-335`; `workspace.rs:33` |
| P3b-7 | Straighten | `tilt` and `swing` never reach `converge_v`/`converge_h` | `geometry.converge_v`, `geometry.converge_h` | `frame_geometry.rs:69-103`, `284-350` |
| P3b-8 | Straighten | Rotate/flip chips are painted with no `Sense`; `rotation`, `flip_horizontal` and `flip_vertical` never change | Those three config fields | `frame_geometry.rs:300-327` |
| P3b-9 | Crop | `ratio` and `guide` are view-only; `autocrop_ratio` and a guide field never change | `geometry.autocrop_ratio` and the crop guide | `frame_geometry.rs:259-264` |
| P3b-10 | Crop | `settings_open` is forced `true` each frame | A persisted fold state | `frame_geometry.rs:265` |
| P3b-11 | Retouch | `heal`, `scratch`, `line`, `clone` toggles are local view flags; `store` writes no retouch field for them | The tool selection, or disable until the tool exists | `frame_finish.rs:281-312`, `195-260` |
| P3b-12 | Retouch | `overlay` selection is never stored | The overlay role, or make it view-only and disable | `frame_finish.rs:284-287` |
| P3b-13 | Metadata | `Analog Gear`, `Capture`, `Exposure` cards return no changed value, so a typed date/place/exposure is not saved | `store` after every editable card | `metadata.rs:288-358`, `189-225` |
| P3b-14 | Export | `input_icc` is a static `None` choice | `export.icc_input_path` | `export.rs:70-72`, `114-143` |
| P3b-15 | Export | `alongside` toggles but is not written by `store`, and `contact_sheet` is never edited | The correct sidecar/contact-sheet field, or remove the controls | `export.rs:144-177`, `311-324` |

### 9.6 P4 — dialogs, shortcuts and cross-cutting gaps

| # | Gap | Evidence | Class |
|---|---|---|---|
| P4-1 | **No dialogs are wired.** The plan's E6 list has no Rust entry point: preferences, export presets, roll settings, gear catalog, sensor/chart calibration, contact sheet, command palette, tutorial and updates. | `negpy_egui/desktop/src` has no dialogs module; plan `E6` | Missing |
| P4-2 | **No shortcuts or registry.** There is no `shortcut_registry`/`keyboard_shortcuts`; `Ctrl+K` is only painted text. This blocks the Find field and every toolbar/tool action promised by `docs/KEYBOARD.md`. | `app.rs`/`rail.rs`; plan `E4` | Missing |
| P4-3 | **No global event/command loop.** The backend `render`, `event`, `channel` and `RenderService` APIs exist, but the shell never starts a worker or consumes `BackendEvent`; all edits are synchronous and local. | `negpy_egui/backend/src/render.rs`, `event.rs`; `desktop/src/app.rs` | Missing |
| P4-4 | **Change detection is not centralized.** Cards return a `bool` and mark dirty, but the shell never compares the config hash or invalidates the texture; there is no single “edit changed → render” seam. | `rail.rs:231-260`; `edit.rs:56-98` | Missing |
| P4-5 | **No UX tests for dead wiring.** The existing tests assert painting and round-tripping of state, not that a click reaches a service. This allowed the P0–P3 gaps to pass. | `*_test` sections in `rail/*.rs`; `cargo test` scope | Missing |

### 9.7 Audit method for the next pass

Run these checks on every new panel before it is accepted:

1. **Click path:** a control that is painted must be an `egui::Response` from
   `ui.interact`/`ui.add` or a widget factory; a painted rect is not a control.
2. **Field path:** the control's `show` return value must reach `store`, and
   `store` must write the config field the label names. Trace it with `rg` from
   the label to the field.
3. **Read path:** what the card displays must come from the frame, the edit or a
   document; a constant or a fixture string is a gap until deliberate.
4. **State path:** every open/fold/selection state must live in a page struct and
   survive a repaint.
5. **Service path:** every action that changes files, pixels or the store must
   call a backend service and report success or failure.
6. **Regression test:** add a test that fails when the click, field or service
   path is removed. A paint-only test does not count.

### 9.8 Recommended fix order

1. **Shell session spine:** select a profile, load the roll, pick the first frame,
   populate the strip, and show a real no-roll state. This unblocks import and
   removes the placeholder photograph.
2. **Selection and render seam:** frame selection, edit reload/save on switch, and
   one render request per config change, coalesced through `RenderService`.
3. **Analysis feed:** pass render metrics into the Analysis chart and readouts.
4. **Section state:** move every section's `open` into its page state and add
   toggle persistence tests.
5. **Action wiring:** connect toolbar, jump/navigation, import open, export, scan
   and the P3 actions to backend services in workflow order.
6. **Dialogs and shortcuts:** add the E6/E4 surfaces only after their actions exist.
7. **Golden coverage:** add empty-workspace and imported-roll captures before
   the dialog set.

**Acceptance for this register:** every P0/P1 row has a regression test that
fails before the fix; every P2/P3 action either reaches a service or is visibly
disabled with the reason.

### 9.9 Source audit, 2026-11-21 (post-batch)

This pass was read from `negpy_egui/backend/src` and `negpy_egui/desktop/src`
after the ten-commit batch. It supersedes the baseline rows it names. A finding
is listed only when the source shows the gap; the earlier rows stay as history.

**Closed since the baseline**

- Crop confirm and Auto Crop redraw: the worker returns the request hash and the
  painted config hash, and the shell accepts only the request in flight.
- Output histogram: the worker bins the display frame and the Filtration, Tone
  and Analysis views paint it. The Analysis Log pill changes the scale.
- Card state and the tab collapse: each page owns its collapsed flag and keeps
  its card states across a repaint.
- Retouch: preview and export bake IR, dust, hair, manual heal, scratch and clone
  before the CPU engine. Heal, Line and Clone have canvas gestures.
- Local masks: unlimited stops, grade and flash reach the print. The canvas
  selects a mask and moves its vertices.
- Export: the sidecar toggle writes `.negpy`, and Input/Output ICC profiles load
  from the user ICC folder.
- Scan: the Scan button, device discovery, Folder as Roll and the write path are
  connected to the scanner worker.

**Backend (P0 risk)**

| # | Gap | Evidence | Class |
|---|---|---|---|
| A0-1 | **No GPU render path.** The Python tree has 33 WGSL shaders. The Rust workspace has none and no backend module links `wgpu`. Every GPU-only stage is absent; the shell draws egui textures from the CPU result. | `find negpy -path '*/shaders/*.wgsl'` = 33; `find negpy_egui -name '*.wgsl'` = 0; no `wgpu` use in `negpy_egui/backend/src` | Missing |
| A0-2 | **Tone-limited local masks do not reach the print.** The print curve accepts a keyed alpha plane and per-mask parameters, but the engine passes neither and leaves `keys: None`. | `backend/src/features/exposure/print.rs:324`; `backend/src/render_engine.rs:707`; `backend/src/features/local/processor.rs` | Missing |
| A0-3 | **The database saves each edit, but no ordinary save mirrors the `.negpy` sidecar.** The writer exists and export calls it; the edit save path writes the database only. | `desktop/src/edit.rs`; `desktop/src/export_worker.rs:128`; no sidecar call in `FrameEdit::save` | Local |
| A0-4 | **No capture adapter is connected.** The capture service and its hardware traits exist; the Scan tab shows a Camera choice but has no Rust camera/light session or capture worker. | `backend/src/services/capture/service.rs`; `desktop/src/view/rail/scan.rs`; no capture worker under `desktop/src` | Stub |
| A0-5 | **The extra analysis scopes are not binned.** Waveform, parade, vectorscope and density histogram kernels exist in the backend, but the render returns only the output histogram. | `backend/src/features/exposure/analysis.rs:626-716`; `desktop/src/render_worker.rs:133` | Missing |

**UI (P1 risk)**

| # | Gap | Evidence | Class |
|---|---|---|---|
| A1-1 | **The toolbar is mostly painted.** Photo/Roll mode, zoom, rotate/flip, Tools, Undo/Redo, Print and Roll Average are drawn without a response. | `desktop/src/app.rs:1586` (`paint_toolbar`); controls at `1650-1844` | Dead |
| A1-2 | **Three card actions are painted but dead.** Crop All Frames, Square to Film and Drop Empty Corners have no request or service call. | `desktop/src/view/rail/frame_geometry.rs:354`, `:423`; `desktop/src/view/rail/frame_finish.rs:378` | Dead |
| A1-3 | **Prescan and Scan as Roll have no worker.** They are now disabled with a reason, but the workflow is absent. | `desktop/src/view/rail/scan.rs:302-306`, `:392-414` | Missing |
| A1-4 | **The Find field is a painted placeholder.** It has `Sense::hover`, static text and a printed `Ctrl+K`. No text field, command palette or shortcut dispatch exists. | `desktop/src/view/rail.rs:891-925` | Dead |
| A1-5 | **The top-bar recipe is constant text.** The file name comes from the frame, but the recipe line is a fixed string rather than the frame config. | `desktop/src/app.rs:1526` | Stub |
| A1-6 | **No dialog set is wired.** The Contact Sheet dialog is live; preferences, export presets, roll settings, gear catalog, calibration, command palette, tutorial and updates have no Rust entry point. | `desktop/src` has `view/contact_sheet_dialog.rs` only; plan E6 | Missing |
| A1-7 | **No shortcut registry or keyboard dispatch exists.** Copy reads no `key_for`; the toolbar and card actions have no bindings. | `desktop/src`; plan E4 | Missing |

**Histogram and masks (P2 risk)**

| # | Gap | Evidence | Class |
|---|---|---|---|
| A2-1 | **The mask picker edits an unlimited mask only.** The Dodge & Burn card writes stops, grade, flash, feather and zone; it cannot set the `MaskKey` or enable a keyed mask. | `desktop/src/view/rail/frame_tone.rs` local store | Local |
| A2-2 | **Oval and card masks cannot be selected.** The hit test treats every mask as a polygon and ignores a two-point card. Once selected, the handles follow the raw control points rather than the oval outline. | `desktop/src/view/canvas.rs` `hit_mask` and `local_tool` | Local |
| A2-3 | **The Analysis card has one histogram.** It paints the output histogram and the sampled curve. The density histogram, waveform, parade and vectorscope have no view or render metrics. | `desktop/src/view/rail.rs:553`; `desktop/src/render_worker.rs:133` | Missing |
| A2-4 | **Readout clip marks are not fed.** The chart paints the wedge and the curve; it does not draw the shadow/highlight clip marks from the histogram bins. | `desktop/src/view/rail.rs:537-585` | Stub |

**Audit gate**

Apply the six checks in §9.7 to every row. A fix needs a regression test that
fails before the change. A row that cannot be fixed in the batch is disabled with
a reason or left listed here; it is not silently painted.

### 9.10 Source audit, 2026-11-21 (second batch)

Read from `negpy_egui/backend/src` and `negpy_egui/desktop/src` after commits
`62a3c939`..`0b08c9a1`. It closes five §9.9 rows and leaves the rest current.

**Closed**

- A0-2: tone-limited masks reach the print. `local::processor` exposes the keyed
  alphas and raw grade deltas, `placement::key_edges` inverts the achromatic
  curve, and `PrintRenderSettings` carries `keys`.
- A0-5: the render worker bins the waveform, parade, vectorscope and density
  histogram; `view/widgets/scopes.rs` paints the first three.
- A2-3: the Analysis card switches between Curve, Waveform, Parade and
  Vectorscope; the curve view also paints the density histogram.
- A2-4: the curve chart marks a channel that clips at either histogram edge.
- A1-2 (partly): Square to Film runs `squared_geometry`; `Drop Empty Corners`
  was retired because no Python control backs it.
- A1-1 (partly): Photo/Roll mode, rotate/flip and Undo/Redo reach the edit.
- A2-1: the Dodge & Burn card writes `MaskKey` and gates the zone rail on it.
- A2-2: an oval selects inside its outline, a card near its axis.

**Closed in the third batch (2026-11-21)**

- B1-1: the toolbar zoom preset, the wheel and a double-click move the
  fit-relative scale, and the readout prints the true pixel zoom.
- B1-2 (partly): Tools opens a menu over the canvas tools and arms the chosen
  tool. Print opens the Export form. A native print dialog has no Rust entry
  point.
- B1-3: the toolbar's Roll Average applies or clears Luma, Color and Cast on
  the open frame.
- B1-4: `desktop::crop_worker` detects each strip frame, resolves the roll
  crops once, applies each rect and keeps a frame edited during the run.
- B1-5: the Find field takes Ctrl+K, filters the shell's cards and actions,
  moves with Up/Down and opens the active row on Enter.
- B1-6: the top-bar recipe reads the frame's paper, Print Density and grade.
- B1-7: `view::shortcuts` holds the keys and the actions they dispatch.

**Still open**

| # | Gap | Evidence | Class |
|---|---|---|---|
| B1-2 | **The native Print dialog is missing.** Print opens the Export form; no OS print dialog exists. | `desktop/src/app.rs` toolbar | Missing |
| B1-8 | **Camera capture is unconnected.** The capture service is ported; there is no capture worker or hardware session. | `backend/src/services/capture/`; `desktop/src/view/rail/scan.rs` | Stub |
| B1-9 | **No GPU render path.** No `wgpu` backend or WGSL shader exists; this is a multi-batch epic. | `negpy_egui` has no `.wgsl` or `wgpu` use | Missing |

**Third-batch source audit (2026-11-21)**

- Camera capture is a major blocker. `services::capture::hardware` holds only the
  `Camera` and `LightSource` traits; there is no gphoto2, Scanlight serial or
  simulated desktop adapter, no capture worker and no session state. Wiring the
  card would paint a dead control, as §9.8 forbids.
- The Scan card still disables Prescan and Scan as Roll for the same reason:
  their workers are not ported.
- A GPU render path still needs a device adapter, the 33 WGSL shaders, stage
  wiring and parity tests. It is a multi-batch epic, not one commit.
- `cargo check --workspace`, `cargo test --workspace` and
  `cargo fmt --all --check` pass after the ten commits. Clippy remains blocked
  by the existing `manual_is_multiple_of` and `too_many_arguments` lints.

The A0-3 row stays as written: a sidecar is written at export time, matching
Python, and an ordinary database save does not mirror one.


---

### 9.11 Source audit, 2026-11-21 (fourth batch)

Read from `negpy_egui/backend/src` and `negpy_egui/desktop/src` after commits
`531c33fa`..`3d1e8645`. The batch closed nine §9.10 and §9.4 rows.

**Closed**

- B1-8 / A1-3 are unchanged: camera capture still has only the `Camera` and
  `LightSource` traits, with no hardware adapter or capture worker. A human
  must choose the target.
- B1-9 / A0-1 are unchanged: the workspace has no WGSL shader and no `wgpu`
  backend, so the GPU path remains a multi-batch epic.
- P3-4 now closes calibration: the Calibration card lists the crosstalk
  profiles and stores the matrix, process and strength. The profile gallery
  and matrix editor have no Rust entry point.
- P3-15 / P3-17 partly close: the Metadata Presets page saves, applies and
  deletes through the settings catalog. It applies only to the open frame.
- P3-14 partly closes: the Export Presets page selects, renames, enables and
  deletes a saved preset. A preset still cannot run on several frames.
- P3-2 partly closes: the Light Table Keep, Reject, Auto Crop and Export
  buttons respond, and Keep and Reject mark the open frame only.
- P3-9 / P3b-9 partly close: the Crop Guide choice persists and its guide
  paints inside the crop box; the Ratio crosshair measures the closest
  standard aspect ratio.
- P3b-14 and P3b-15 partly close: the File page covers the bit depth, TIFF,
  PNG, JXL and WebP encoder fields, the print size, dpi and absolute path.
- P2-2 partly closes: the Contact Sheet page count comes from the sheet
  layout and Next stops at the last page. Pick Frames is still painted without
  a response.

**Still open after the batch**

| # | Gap | Evidence | Class |
|---|---|---|---|
| C1-1 | **The Frame Favorites tab has no real page.** It still shows the `Presets` placeholder from `show_collapsed`; no favourite slider list or editor exists. | `desktop/src/view/rail.rs` `frame_cards("favourites")` | Missing |
| C1-2 | **The Contact Sheet Footer's Pick Frames is dead.** The button is painted and dropped, and no per-sheet frame picker or left-out state exists. | `desktop/src/view/contact_sheet_dialog.rs:178` | Dead |
| C1-3 | **Roll Analysis Detect is painted and dropped.** The header action in `show_roll_analysis_body` has no detector, scene list or accept step. | `desktop/src/view/rail.rs:860` | Dead |
| C1-4 | **Metadata presets cannot reach a selection or the roll.** Apply writes only the open edit, while the Python dialog asks for a scope. | `desktop/src/app.rs` `apply_metadata_preset` | Local |
| C1-5 | **A saved export preset cannot be run.** The Presets page can enable, rename, delete and apply a preset to the form; no preset export runs. | `desktop/src/app.rs` `update_export_presets` | Local |
| C1-6 | **Hand-picked Dmin and Dmax remain unconnected.** The Metering card toggles its pick mode, but the canvas has no pick action and no samples the normalized frame. | `desktop/src/view/rail/metering.rs`; `desktop/src/view/canvas.rs` | Missing |
| C1-7 | **Card placeholders remain.** `show_collapsed` still paints `This card has no controls yet.` for cards with no Rust page. | `desktop/src/view/rail.rs:779` | Missing |

**Verification**

- `cargo check --workspace`, `cargo test --workspace` and
  `cargo fmt --all --check` pass after the ten commits.
- The backend dependency tree has no `pyo3`, `numpy`, `egui` or `eframe`.
- The workspace holds no WGSL shader.
- Clippy remains blocked by the existing `manual_is_multiple_of` and
  `too_many_arguments` lints.

**Fifth-batch source audit (2026-11-21)**

Read from `negpy_egui/backend/src` and `negpy_egui/desktop/src` after commits
`838e2ed3`..`45c30176`. The batch closed C1-1 through C1-7.

**Closed**

- C1-1: the Favorites page is its own card over seven Exposure sliders, stored
  under `favourite_sliders`.
- C1-2: Pick Frames lists the strip and leaves a cleared frame out of the
  entries and the page plan.
- C1-3: Detect measures each strip frame's neutral axis and lists the groups;
  Accept writes them as scenes.
- C1-4: Metadata Presets take an Apply to Frame or Roll choice.
- C1-5: Export to Preset runs the selected preset on the filtered strip.
- C1-6: Dmin and Dmax clicks sample the unmixed log patch, store the bound and
  its point, and clear with a button.
- C1-7: the placeholder card is gone; History shows its two real cards.

**Still open**

| # | Gap | Evidence | Class |
|---|---|---|---|
| C2-1 | **A bounds pick reads the uncropped frame.** The canvas shows the cropped render, but `store_bounds_pick` samples the geometry-processed frame, so a click moves when a crop exists. Python's pick tools show the uncropped frame. | `desktop/src/app.rs` `store_bounds_pick` | Local |
| C2-2 | **Detect decodes the strip on the UI thread.** It runs a bounded decode and analysis per frame inside `update`, which the architecture rule forbids. | `desktop/src/app.rs` `detect_scenes` | Stub |
| C2-3 | **Accepted scenes are not analyzed.** The group becomes a roll scene, but no Scene Analysis runs and the frames stay on the roll baseline until Reanalyze. | `desktop/src/app.rs` `accept_scenes` | Local |
| C2-4 | **Metadata apply to Roll ignores selection and card locks.** It writes every other roll frame, not a multi-frame selection, and does not lock a card that had diverged. | `desktop/src/app.rs` `apply_preset_to_roll` | Local |
| C2-5 | **The sheet picker does not seed from rejected frames and lists names.** A frame marked `excluded` still prints, and only the name list is clickable, not the sheet tile. | `desktop/src/view/contact_sheet_dialog.rs` | Local |
| C2-6 | **Export to Preset ignores overwrite.** `write_export` replaces the destination file; the preset's `overwrite` flag is not read. | `desktop/src/export_worker.rs` | Local |
| C2-7 | **The Favorites picker has no order.** The Python editor drags to reorder; the Rust picker only toggles, and reordering changes the list order on save. | `desktop/src/view/rail/favourites.rs` | Missing |
| C2-8 | **Camera capture and the GPU path are unchanged.** A human must choose the hardware target; the GPU engine remains a multi-batch epic. | §9.11 B1-8, B1-9 | Blocked |

**Verification**

- `cargo check --workspace`, `cargo test --workspace` and
  `cargo fmt --all --check` pass after the ten commits.
- The histogram path is unchanged and verified end to end: the worker bins the
  display frame and the density metrics, the rail stores them, and the Analysis
  card paints the curve, waveform, parade and vectorscope.
- No file under `negpy/`, `docs/` or `negpy_core/` changed.

**Sixth-batch source audit (2026-11-21)**

Read from `negpy_egui/backend/src` after commits `ab73742e`..`770e58e8`. This
batch starts the GPU epic. It does not close B1-9, because the desktop render
worker still runs the CPU engine.

**Closed**

- The device adapter opens one adapter and device per process, and stores a
  failed open instead of retrying (`infrastructure/gpu/device.rs`).
- Texture, buffer and shader-cache helpers mirror the Python resources. RGB
  uploads pad alpha, and a readback returns RGBA floats.
- Five shaders are ported: geometry transform, auto-crop evidence, normalization,
  the display encode and the density histogram.
- `services/rendering/gpu_engine.rs` runs geometry, normalization, the display
  encode and the density histogram over one texture chain. The chain reads the
  bounds the CPU base measured.
- Parity tests compare each ported stage with its CPU twin: geometry, the
  normalization stretch with a density stage and crossover table, the display
  encode, the density bins, border evidence and the full prefix. Each test skips
  when no adapter opens.
- The engine reads `use_gpu`, so `override.toml`'s CPU backend and Preferences
  keep the device closed.

**Still open (GPU epic)**

| # | Gap | Evidence | Class |
|---|---|---|---|
| G-1 | **Twenty-eight of the 33 upstream shaders are unported.** The exposure curve, CLAHE, Lab, local masks, LUT, toning, finish, transfer and the scope shaders have no Rust pass. | `find negpy -path '*/shaders/*.wgsl'` = 33; `backend/src/features/**/shaders` holds 5 | Missing |
| G-2 | **The desktop render worker does not call the GPU chain.** It still runs `DarkroomEngine`; `RenderParams.gpu_enabled` has no reader. | `desktop/src/render_worker.rs` | Local |
| G-3 | **The GPU chain is a prefix, not a replacement.** It stops after the display encode, so crop, exposure, Lab, toning and finish stay on the CPU. | `backend/src/services/rendering/gpu_engine.rs` | Local |
| G-4 | **No stage caching, texture pool, tiling or readback pipelining.** Each call allocates textures and waits for the queue. | `gpu_engine.rs`; Python `_tex_cache`, `_process_tiled` | Missing |
| G-5 | **No GPU golden pins.** A driver change can move the bits unnoticed; the CPU pins do not cover the shaders. | `tests/` | Missing |
| G-6 | **No camera-path or per-channel parity case.** Bounds come from the CPU, so the meters themselves are untested on the GPU. | `gpu_normalization_parity.rs` | Missing |

**Verification**

- `cargo check --workspace` and `cargo test --workspace` pass after the ten
  commits.
- The GPU parity tests pass on this machine's Vulkan adapter and skip on a host
  without one.
- Clippy remains blocked by the existing `while_immutable_condition`,
  `approx_constant` and `eq_op` lints. The new files add only
  `chunks_exact_to_as_chunks` warnings.
- No file under `negpy/`, `docs/` or `negpy_core/` changed.

---

## 9.12 GPU batch audit, 2026-11-21 (seventh batch)

Read from `negpy_egui/backend/src` and `negpy_egui/desktop/src` after commits
`857899e6`..`f82cfcad`. This batch closed the GPU crop, print, vignette, worker,
texture-pool, prefix-cache and pin rows. It stopped before commit 9 at the CLAHE
parity blocker.

**Closed**

- G-3: the print chain runs the exposure curve and matches `render_negative`.
- G-2: the render worker uses the chain for a print frame with no local map,
  print LUT or carrier, and keeps the CPU engine otherwise.
- G-4 (partly): the engine reuses textures by size and keeps the geometry and
  normalization textures until the source or prefix key changes. Tiling and
  readback pipelining remain open.
- G-5 / G-6: the print and vignette shaders have pinned output hashes, and a
  transfer-path camera matrix matches the core.

**Blocker (resolved in the eighth batch)**

- The tile rules already agree. `h.div_ceil(8)` equals `(h + 7) / 8`, and
  `wid * tile_size` with `end = min(start + tile, dims)` assigns exactly the
  pixels with `x / tsx == wid`. The real defect was `clahe_cdf.wgsl` dividing a
  zero total by zero on a dimension that is not a multiple of 8: the empty tile
  row produced a NaN CDF, and the remap blended it into the frame edge. The
  port clamps the denominator to one, as the core does.
- The GPU worker measures the base through `normalize_negative`'s CPU pass,
  because the meters are not on the GPU yet. It now passes the crop window so
  its bounds match the CPU engine.
- A cropped GPU frame was cropped twice, once inside `run_print_with` and once
  in the tail. The chain keeps the full frame through CLAHE and Lab, and the
  toning pass performs the one crop.

### 9.13 GPU tail audit, 2026-11-22 (eighth batch)

Read from `negpy_egui/backend/src` and `negpy_egui/desktop/src` after commits
`64a962ab`..`f62249e0`. This batch closed the CLAHE blocker and moved the rest
of the creative tail onto the device except finish and alt process.

**Closed**

- G-1 (partly): CLAHE, Lab with its sharpen and chroma preconditions, and
  toning have passes. Nineteen of the 33 upstream shaders are ported.
- G-3: the GPU tail runs print, CLAHE, Lab and toning; a cropped frame and a
  B&W frame with a sepia bath match the CPU engine.
- G-4 (partly): the Lab and toning passes take textures from the size-keyed
  pool. Tiling and readback pipelining remain open.
- G-5: the CLAHE, Lab and toning shaders have pinned output hashes.

**Still open (GPU epic)**

| # | Gap | Evidence | Class |
|---|---|---|---|
| G-1 | **Fourteen upstream shaders are unported.** Finish, carrier and layout, lith, cyanotype, the Sabattier passes, the RL chain, and the metrics, waveform, vectorscope and color-histogram scopes have no Rust pass. | `negpy_egui/backend/src/features/**/shaders` holds 19 | Missing |
| G-4 | **No tiling or readback pipelining.** `GpuTexture::readback` still waits on the queue per frame. | `resources.rs`; Python `_process_tiled` | Missing |
| G-6 | **No camera-path or per-channel parity case for the new passes.** Bounds come from the CPU. | `gpu_normalization_camera.rs` | Missing |
| G-7 | **An RL-sharpened frame silently runs USM on the GPU.** `lab_shader::run` branches to the USM path for every method and `gpu_eligible` does not reject RL. | `lab_shader.rs`; `gpu_render.rs` | Local |

**Verification**

- `cargo check --workspace`, `cargo test --workspace` and
  `cargo fmt --all --check` pass after the ten commits.
- The GPU parity tests ran on this machine's Vulkan adapter and skip on a host
  without one.
- `timing::tests` is a wall-clock test that flakes under load and passes alone.

---

## 10. Crop Interaction Plan (historical)

> **Historical.** The ninth batch closed the crop rows in `MIGRATION_PLAN.md`.
> This section records the plan that produced them, not an open task.

This plan brings the Rust crop canvas up to the interaction the Python app
already describes. It does not change `docs/` or the Python implementation.

### 10.1 Goal

- The crop outline stays a rectangle in every drag direction.
- The crop is easier to see while the user drags it.
- Right-click or Enter keeps the crop.
- A pending crop paints on every pointer move but reaches the edit and the
  render worker only on confirm.

### 10.2 Current state

`view/canvas.rs::crop_tool` holds a typed `CropDrag` and returns a `CropAction`.
It paints the pending rectangle during a drag, and it accepts Enter or a
right-click only while Draw is on. `ShellHits.crop` carries `Pending` and
`Confirm`. `NegFeApp` writes the rect and clears the Auto Crop key.

The local mask tool claimed the same drag while it was Off, and
`GeometryState::load` cleared Draw whenever a stored crop existed; both are
fixed. The redraw seam is now explicit: the worker returns the request hash and
the painted config hash, and the shell accepts a render only for the request in
flight. A test drives a confirmed crop and an armed Auto Crop through that seam.

The Rust shell has no shortcut registry. Enter must stay canvas-local so it does
not compete with text fields or the future global shortcut layer. Text-focus
detection must test for a text cursor, not any focused widget.

### 10.3 Interaction contract

- **Draw on** makes the photo a crosshair surface.
- A press starts a crop drag at the clamped pointer position.
- A pointer move updates a pending normalized rectangle.
- A rectangle below `MIN_CROP` in either dimension paints while dragging but
  does not replace the stored crop on confirm.
- A release keeps the pending rectangle and leaves Draw on.
- Enter confirms the pending rectangle, else the stored rectangle. A focused
  button never blocks it; a focused text field with a cursor does.
- Right-click inside the photo confirms the pending rectangle, else the stored
  rectangle. It never starts a drag.
- Confirm writes the rectangle once, sets `crop_from_auto = false`, clears
  `crop_detect_key` and `autocrop_squared`, marks the edit dirty, requests one
  render, and turns Draw off.
- Escape is the existing tool-cancel path. It is not part of this task.

### 10.4 Visual contract

- Keep the outside shade at `with_alpha(mark_shadow, 110)`.
- Draw a dark outer stroke and a light inner stroke around the pending and stored
  rectangle, so the edge reads on light and dark photographs. Use theme tokens;
  add one crop-accent token only if no existing token passes the contrast tests.
- Draw four corner handles. They are larger during a drag and at rest than the
  current 6 px square.
- Keep the rectangle rectangular while the drag is small: paint the pending box,
  do not collapse it to a line.
- Show one short canvas hint while Draw is on: “Drag a crop. Right-click or
  Enter keeps it.” Keep the text in the canvas overlay, not in the Crop card.
- No new font size, spacing value or private widget style. Use the existing
  `Theme` tokens and the established card and fold conventions.

### 10.5 State and seams

`view/canvas.rs` holds the typed drag state and action:

```rust
pub struct CropDrag {
    pub start: egui::Pos2,
    pub pending: [f64; 4],
}

pub enum CropAction {
    None,
    Pending([f64; 4]),
    Confirm([f64; 4]),
}
```

`crop_tool` returns `CropAction`. `paint_shell` maps it to `ShellHits.crop`.
`NegFeApp::draw` is the only place that writes the rect and requests a render.
The pending rect lives in `NegFeApp`, so it survives a repaint and can be
confirmed by a key. Crop interaction comes from Draw, while the stored rect
controls only whether the box paints.

Enter handling:

```rust
let enter = ui.input(|input| input.key_pressed(egui::Key::Enter));
let editing_text = text_has_cursor(ui);
```

Confirm only when `draw && enter && !editing_text`. Right-click uses the photo
response's `secondary_clicked()`.

### 10.6 Commit sequence

Each commit must pass its targeted tests, `cargo check --workspace` and
`cargo fmt --all --check`. The tenth commit of a batch runs the §8.7 source
audit, then `cargo test --workspace`.

| # | Subject | Scope | Test |
|---|---|---|---|
| 1 | `frame: keep a typed crop drag` | Move the drag start and pending rect into `CropDrag`; keep the current commit behavior. | A drag from each corner normalizes to the same ordered rect; points outside the photo clamp. |
| 2 | `frame: draw the crop box while dragging` | Paint the pending rect, outer and inner strokes, larger handles and the canvas hint. | A pointer press/move paints a pending box and leaves the stored edit unchanged. |
| 3 | `frame: confirm the crop with Enter or right-click` | Add `CropAction::Confirm`, Enter handling with the text-focus guard, right-click confirmation, one write and Draw off. | Enter and right-click each produce one dirty edit and one render request; a focused text field does not confirm. |
| 4 | `frame: redraw after a crop confirms` (done) | Track the request that produced the visible texture. Accept only a render whose request hash is the one in flight. | A render is rejected for an older generation or a different request hash. |
| 5 | `frame: redraw after Auto Crop freezes` (done) | Freeze the resolved geometry before the texture is recorded, then show that texture once. | An armed Auto Crop freezes before its texture is accepted and submits no second render. |

### 10.7 Acceptance

- A drag in any direction shows a rectangle, not a line.
- The outline remains visible over a white and a black photograph.
- Right-click and Enter both keep the same rectangle.
- A drag below `MIN_CROP` paints but does not replace the stored crop.
- One confirm writes one config change and submits one render request.
- The canvas shows a texture whose config hash contains the confirmed crop.
- An armed Auto Crop freezes once and redraws once with the frozen rect.
- The existing crop, Auto Crop, ratio-reshape and roll-baseline tests stay green.
- No file under `negpy/`, `docs/` or `negpy_core/` changes.
