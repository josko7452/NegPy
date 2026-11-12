# Development Ledger
**Last Updated:** 2026-11-12 21:20 | **Last Commit:** `_ backend: add the workspace config flat namespace`

## Current Goal
Port the `negpy/` Python layer to Rust in `negpy_egui/`, bottom-up.
**Source of truth: `negpy_egui/AGENTS.md`** (read it first each session).
`MIGRATION_PLAN.md` is advisory. Phase B and C (event contract) are complete.
  **Phase D1 is complete. D2 is complete**: `types`, `migrations` and the full
  `models` DTO set landed; **D3 (`kernel.system.config`) is next**.

## Completed (Recent)
- A1–A5 Phase A scaffolding; B1–B3 `negpy_core` verify-only audit; C1–C4 the frozen
  event/command contract, `std::sync::mpsc` channels and the render thread.
- D1a–d `kernel/system/{paths,user_directory,i18n,text,logging,override_config}.rs`
  and `domain/types.rs` (`AppConfig`). 58 tests.
- D2 `domain/`: `migrations.rs` (key renames, value rewrites, dropped keys);
  `models/{string_enum,process,exposure,geometry,lab,local,retouch,altprocess,toning,
  finish,flatfield,export,metadata,hdr,rgbscan,stitch,workspace}.rs` and `tokens.rs`.
  `WorkspaceConfig` flattens to one key namespace via `to_dict`/`from_flat_dict`.
  46 tests, incl. a 0.63.0 sidecar fixture round-trip. 134 tests total, green.

## Next Steps (Ordered)
1. [ ] D3 `kernel.system.config` (TOML, shipped defaults) + `override` sync
2. [ ] D4–D13 Layer 2 services, in order (see MIGRATION_PLAN.md §7.4)

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
- D2: crates.io is offline; `serde`, `serde_json` (preserve_order) and `uuid` are in the
  local registry. No `sha2`/`blake3`, so `composite_token` uses FNV-1a (see D2e). The flat
  `to_dict` emits JSON in section order; Python emits with `json.dumps` defaults.
