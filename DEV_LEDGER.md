# Development Ledger
**Last Updated:** 2026-11-12 17:45 | **Last Commit:** `_ backend: add i18n and text helpers`

## Current Goal
Port the `negpy/` Python layer to Rust in `negpy_egui/`, bottom-up.
**Source of truth: `negpy_egui/AGENTS.md`** (read it first each session).
`MIGRATION_PLAN.md` is advisory. Phase B and C (event contract) are complete.
  Phase D1 is in progress: paths, user dir, i18n and text landed; logging and
  override next.

## Completed (Recent)
- A1–A5 Phase A scaffolding (workspace, crates, CI `negpy_egui` job, upstream clean).
- B1–B3 `negpy_core` visibility audit: verify-only, every pure kernel already `pub`,
  so no promotion; `histogram::Samples` and the config structs are `pub`. Smoke test
  calls `stats::percentiles`, `color::rgb_to_lab`, `convert::to_uint` on `&[f32]`.
- C1 `BackendEvent`/`BackendCommand` enums in `backend/src/event.rs`; the 456-signal
  bucket table in `negpy_egui/docs/EVENT_CONTRACT.md` (200 EV, 48 CMD, 208 GUI).
- C2 `backend/src/channel.rs`: `std::sync::mpsc` pair (commands in, events out) with a
  `Wake` callback the GUI installs for `request_repaint`. Round-trip test passes.
- C3 `backend/src/render.rs`: render thread with a one-slot latest-wins queue, the
  `render_size` proxy rule (≤1600 px while dragging, full-res otherwise), and tests for
  coalescing, close semantics and the size rule.
- C4 `EVENT_CONTRACT.md` gains the class mapping: each Python controller/session/worker
  class to its `BackendEvent`/`BackendCommand`/GUI-local home.
- D1a `backend/src/kernel/system/{paths,user_directory}.rs`: resource and user-dir
  resolution, atomic replace, safe unlink, umask mode. 15 tests ported.
- D1b `backend/src/kernel/system/{i18n,text}.rs`: language resolution, the catalog
  loader with placeholder validation, the `str.format` subset, `plural`/`count_of`/
  `human_bytes`. 17 tests ported.

## Next Steps (Ordered)
1. [ ] D1 `kernel.system.{logging,override}`; paths, i18n, text done
2. [ ] D2–D13 Layer 2 services, in order (see MIGRATION_PLAN.md §7.4)
3. [ ] E1–E8 Layer 3 egui UI, in order (see MIGRATION_PLAN.md §7.5)

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
- Phase C MUST land before porting `controller.py`/`session.py`.
- Divergence recorded (plan §8.6): `.git/info/exclude` ignores the migration docs;
  `DEV_LEDGER.md` is force-added. Anchoring `/AGENTS.md` needs a `.git/` write
  the sandbox forbids. Human action pending.
- D1 divergence: i18n embeds the catalogs from `../negpy/translations/` with
  `include_str!` instead of copying them to `backend/src/assets/translations/`.
  One source, no 1.9 MB duplicate; the Python tree is read-only here.
