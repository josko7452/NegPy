# Development Ledger
**Last Updated:** 2026-11-12 20:20 | **Last Commit:** `_ backend: add image-editing config DTOs`

## Current Goal
Port the `negpy/` Python layer to Rust in `negpy_egui/`, bottom-up.
**Source of truth: `negpy_egui/AGENTS.md`** (read it first each session).
`MIGRATION_PLAN.md` is advisory. Phase B and C (event contract) are complete.
  **Phase D1 is complete. D2 is in progress**: `types` and `migrations` landed;
  `models` DTOs are landing; `tokens` follows.

## Completed (Recent)
- A1–A5 Phase A scaffolding (workspace, crates, CI `negpy_egui` job, upstream clean).
- B1–B3 `negpy_core` visibility audit: verify-only, every pure kernel already `pub`,
  so no promotion; `histogram::Samples` and the config structs are `pub`. Smoke test
  calls `stats::percentiles`, `color::rgb_to_lab`, `convert::to_uint` on `&[f32]`.
- C1–C4 `backend/src/{event,channel,render}.rs`: the frozen event/command contract + the
  456-signal table (`negpy_egui/docs/EVENT_CONTRACT.md`), the `std::sync::mpsc` pair with
  a `Wake` for `request_repaint`, and the render thread (latest-wins, ≤1600 px proxy).
- D1a `backend/src/kernel/system/{paths,user_directory}.rs`: resource and user-dir
  resolution, atomic replace, safe unlink, umask mode. 15 tests ported.
- D1b `backend/src/kernel/system/{i18n,text}.rs`: language resolution, the catalog
  loader with placeholder validation, the `str.format` subset, `plural`/`count_of`/
  `human_bytes`. 17 tests ported.
- D1c `backend/src/kernel/system/logging.rs`: `setup_logging` via `fern`, console plus
  a rotating `negpy.log` under the user dir, and the `negpy.<module>` target. 3 tests.
- D1d `backend/src/kernel/system/override_config.rs` + `domain/types.rs`: `override.toml`
  load/parse/apply, the cache recommendation, the stored-preference merge. 23 tests.
  `AppConfig` lands here because override needs it; D2 extends `domain/`.
- D2a `backend/src/domain/migrations.rs`: every key rename, value rewrite and dropped
  key for persisted configs, over a JSON field map. 20 tests.
- D2b `backend/src/domain/models/{string_enum,process,exposure}.rs`: the `string_enum!`
  macro (StrEnum + `_missing_` coercion), Process and Exposure DTOs, the legacy grade
  conversion and the bool->float cast-removal adapter. 9 tests.
- D2c image-editing DTOs: geometry, lab, local, retouch, altprocess, toning, finish,
  flatfield. 8 tests.

## Next Steps (Ordered)
1. [ ] D2 `domain.models` (WorkspaceConfig serde DTOs) + `domain.tokens`
2. [ ] D3–D13 Layer 2 services, in order (see MIGRATION_PLAN.md §7.4)

## Constraints / Gotchas
- 5-commit hard stop per session; update this ledger before every commit.
- Keep this file < 60 lines; overwrite completed items, don't append.
- Tests run from `negpy_egui/` only: `cargo check --workspace && cargo test --workspace`.
- Never edit `negpy/`. Layer 1 = verify-only; a needed `pub` promotion in
  `negpy_core/src/*.rs` is the sole exception (scope `core:`, ask human first).
- `negpy_core` stays its own workspace (path dep, not a member) — no root `Cargo.toml`.
- Env: crates.io is blocked; `cargo` runs here without escalation. No new crates can
  be fetched, so C2 uses `std::sync::mpsc` (AGENTS allows it). `git` writes need
  escalation. `apply_patch` is on PATH (the tool arg form is unavailable).
- `negpy_egui/Cargo.lock` is tracked; regenerable.
- Subject ≤50 chars, body wrapped at 72, no trailing period; split commits on "and".
- Anti-loop: 3 failed fix attempts ⇒ STOP and ask for human guidance.
- Phase C (done) let `controller.py`/`session.py` port against a frozen contract.
- Divergence recorded (plan §8.6): `.git/info/exclude` ignores the migration docs;
  `DEV_LEDGER.md` is force-added. Anchoring `/AGENTS.md` needs a `.git/` write
  the sandbox forbids. Human action pending.
- D1 divergence: i18n embeds the catalogs from `../negpy/translations/` with
  `include_str!` instead of copying them to `backend/src/assets/translations/`.
  One source, no 1.9 MB duplicate; the Python tree is read-only here.
