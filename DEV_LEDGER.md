# Development Ledger
**Last Updated:** 2026-11-13 00:05 | **Last Commit:** `6d6dea19 backend: add roll fork identities`

## Current Goal
Port the `negpy/` Python layer to Rust in `negpy_egui/`, bottom-up.
**Source of truth: `negpy_egui/AGENTS.md`** (read it first each session).
`MIGRATION_PLAN.md` is advisory. Phase B and C (event contract) are complete.
  **D1–D5 storage slices are complete.** Next is D5 roll/composite integration.

## Completed (Recent)
- A–C: workspace, core audit, event contract and render worker.
- D1–D2: kernel/system leaves, `AppConfig`, all config DTOs, migrations.
- D3: shipped config, shipped edit and the stat-keyed TOML cache.
- D5: atomic sidecars, bounds migration, presets and guarded promotion.
- D4: SQLite schema, settings/edits/history/marks/work prints, embeddings,
  local asset store, stats and reset.

## Next Steps (Ordered)
1. [ ] D5 repository helpers for batch reads and path rehoming
2. [ ] D5 roll-wide defaults and card locks
3. [ ] D4 CI dependency-graph parity closeout
4. [ ] D5 sidecar promotion batch parity test
5. [ ] D4 CI dependency-graph parity closeout

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
- Crates: crates.io is reachable from the host, but the coding sandbox has no network and a
  read-only `~/.cargo`. Run `make fetch-crates` from a networked shell after any
  Cargo.toml/Cargo.lock change; it repopulates the shared `~/.cargo/registry` and sandboxed
  builds then resolve from cache. No registry mirror or vendoring is needed.
- D2: `composite_token` hashes with `sha2` (16 hex digits, like Python). The digested bytes
  are canonical JSON, not Python's `repr(astuple)`, so a token is not interchangeable with
  Python's — it is a cache key never persisted, so it only has to be stable here. The flat
  `to_dict` emits JSON in section order; Python emits with `json.dumps` defaults.
