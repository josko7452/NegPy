# Development Ledger
**Last Updated:** 2026-11-12 14:20 | **Last Commit:** `592eb3d9 A2 backend negpy_core dep`

## Current Goal
Stand up the `negpy_egui/` workspace and port the `negpy/` Python layer to Rust
bottom-up. **Source of truth: `negpy_egui/AGENTS.md`** (read it first each
session). `MIGRATION_PLAN.md` is advisory. Start with Phase A (scaffolding).

## Completed (Recent)
- A1 workspace manifest + toolchain pin + empty backend/desktop members.
- A2 backend crate consumes negpy_core (no pyo3/numpy in its tree).
- A3 desktop crate: eframe/egui/wgpu + negpy-backend (no pyo3).

## Next Steps (Ordered)
1. [ ] A4 `backend: add CI dependency-graph gate` (dep: A2, A3)
2. [ ] A5 `plan: record Phase A completion in DEV_LEDGER` (dep: A4)
3. [ ] B1 audit `negpy_core` pub visibility (Layer 1)
4. [ ] C1 define `BackendEvent`/`BackendCommand` enums (BEFORE controller port)
5. [ ] D1–D13 Layer 2 services, in order (see MIGRATION_PLAN.md §7.4)
6. [ ] E1–E8 Layer 3 egui UI, in order (see MIGRATION_PLAN.md §7.5)

## Constraints / Gotchas
- 5-commit hard stop per session; update this ledger before every commit.
- Keep this file < 60 lines; overwrite completed items, don't append.
- Tests run from `negpy_egui/` only: `cargo check --workspace && cargo test --workspace`.
- Never edit `negpy/`. Layer 1 = verify-only; a needed `pub` promotion in
  `negpy_core/src/*.rs` is the sole exception (scope `core:`, ask human first).
- `negpy_core` stays its own workspace (path dep, not a member) — no root `Cargo.toml`.
- Toolchain: pin a `negpy_egui/rust-toolchain.toml` at 1.99 to mirror `negpy_core`.
- Env: crates.io is blocked in the sandbox; run `cargo`/`rustup` with escalation.
  Rust 1.99 toolchain is installed; `git` writes also need escalation here.
- Workspace `Cargo.lock` is tracked (user-approved); it is large but regenerable.
- Subject ≤50 chars, body wrapped at 72, no trailing period; split commits on "and".
- Anti-loop: 3 failed fix attempts ⇒ STOP and ask for human guidance.
- Phase C (event contract) MUST land before porting `controller.py`/`session.py`.
- SOURCE OF TRUTH = `negpy_egui/AGENTS.md`. On conflict, it wins over MIGRATION_PLAN.md.
- Migration docs are git-ignored via `.git/info/exclude` (`AGENTS.md` pattern has no
  slash → matches at any depth). `DEV_LEDGER.md` is committed at A5; the AGENTS.md
  pattern still needs anchoring if the rules file must be tracked (plan §8.6).
