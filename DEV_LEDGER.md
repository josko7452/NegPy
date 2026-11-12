# Development Ledger
**Last Updated:** 2026-11-12 15:10 | **Last Commit:** `_ backend: call negpy_core kernels from a test`

## Current Goal
Port the `negpy/` Python layer to Rust in `negpy_egui/`, bottom-up.
**Source of truth: `negpy_egui/AGENTS.md`** (read it first each session).
`MIGRATION_PLAN.md` is advisory. **Phase B is complete** — start Phase C
(event contract, before the `controller.py`/`session.py` port).

## Completed (Recent)
- A1–A5 Phase A scaffolding (workspace, crates, CI `negpy_egui` job, upstream clean).
- B1–B3 `negpy_core` visibility audit: verify-only, every pure kernel already `pub`,
  so no promotion; `histogram::Samples` and the config structs are `pub`. Smoke test
  calls `stats::percentiles`, `color::rgb_to_lab`, `convert::to_uint` on `&[f32]`.

## Next Steps (Ordered)
1. [ ] C1 `BackendEvent`/`BackendCommand` enums + 456-signal bucket table
2. [ ] C2 channels (commands in, events out) + `request_repaint` wake
3. [ ] C3 render worker thread + latest-wins coalescing + proxy rule
4. [ ] C4 `AppController`/worker → contract mapping doc
5. [ ] D1–D13 Layer 2 services, in order (see MIGRATION_PLAN.md §7.4)
6. [ ] E1–E8 Layer 3 egui UI, in order (see MIGRATION_PLAN.md §7.5)

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
