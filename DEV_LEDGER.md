# Development Ledger
**Last Updated:** 2026-11-12 14:45 | **Last Commit:** `779648c7 A4 CI gate`

## Current Goal
Port the `negpy/` Python layer to Rust in `negpy_egui/`, bottom-up.
**Source of truth: `negpy_egui/AGENTS.md`** (read it first each session).
`MIGRATION_PLAN.md` is advisory. **Phase A (scaffolding) is complete** — start
Phase B (Layer 1 `negpy_core` audit).

## Completed (Recent)
- A1 workspace manifest + 1.99 toolchain pin + empty members.
- A2 backend consumes negpy_core (no pyo3/numpy in its tree).
- A3 desktop crate: eframe/egui/wgpu + one-way negpy-backend dep.
- A4 CI `negpy_egui` job + `make rust-egui` (fmt/clippy/test/headless gate).
- A5 verified upstream untouched: `negpy_core/Cargo.lock`, `flake.nix`,
  `pyproject.toml` unchanged; no edits under `negpy/` or `negpy_core/src/`.

## Next Steps (Ordered)
1. [ ] B1 audit `negpy_core` pub visibility (Layer 1; verify-only by default)
2. [ ] B2 confirm `histogram::Samples`, `zones::Frame`, config types are `pub`
3. [ ] B3 smoke test: backend calls 3 kernels on `&[f32]`
4. [ ] C1 define `BackendEvent`/`BackendCommand` enums (BEFORE controller port)
5. [ ] C2–C4 channels + render worker + mapping doc
6. [ ] D1–D13 Layer 2 services, in order (see MIGRATION_PLAN.md §7.4)
7. [ ] E1–E8 Layer 3 egui UI, in order (see MIGRATION_PLAN.md §7.5)

## Constraints / Gotchas
- 5-commit hard stop per session; update this ledger before every commit.
- Keep this file < 60 lines; overwrite completed items, don't append.
- Tests run from `negpy_egui/` only: `cargo check --workspace && cargo test --workspace`.
- Never edit `negpy/`. Layer 1 = verify-only; a needed `pub` promotion in
  `negpy_core/src/*.rs` is the sole exception (scope `core:`, ask human first).
- `negpy_core` stays its own workspace (path dep, not a member) — no root `Cargo.toml`.
- Env: crates.io is blocked in the sandbox; run `cargo`/`rustup` with escalation.
  Rust 1.99 toolchain is installed; `git` writes also need escalation here.
- Workspace `negpy_egui/Cargo.lock` is tracked (user-approved); regenerable, large.
- Subject ≤50 chars, body wrapped at 72, no trailing period; split commits on "and".
- Anti-loop: 3 failed fix attempts ⇒ STOP and ask for human guidance.
- Phase C (event contract) MUST land before porting `controller.py`/`session.py`.
- Divergence recorded (plan §8.6): `.git/info/exclude` still ignores the migration
  docs. `DEV_LEDGER.md` is now force-added; anchoring `/AGENTS.md` would track the
  rules file but the sandbox cannot write `.git/`. Human action pending.
