# Development Ledger
**Last Updated:** 2026-11-12 22:15 | **Last Commit:** `_ backend: add JSON sidecars`

## Current Goal
Port the `negpy/` Python layer to Rust in `negpy_egui/`, bottom-up.
**Source of truth: `negpy_egui/AGENTS.md`** (read it first each session).
`MIGRATION_PLAN.md` is advisory. Phase B and C (event contract) are complete.
  **Phase D1 is complete. D2 is complete.** D3 is in progress: the shipped app
  config and default edit landed; TOML cache, sidecars, presets and storage
  follow.

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
- D3 `kernel/system/config.rs`: `app_config()` and `default_workspace_config()`;
  path, limit and shipped-default assertions.
- D3 `services/assets/toml_cache.rs`: stat-keyed TOML cache; invalid files
  cache as `None` until the stat changes.
- D5 `services/assets/sidecar.rs`: atomic `.negpy` write, bounds-version
  migration and round-trip; `ProcessConfig::is_local_initialized`.

## Next Steps (Ordered)
1. [ ] D5 `services/assets/presets.rs`
2. [ ] D4 `infrastructure/storage/repository.rs` (SQLite schema and settings rows)
3. [ ] D4/D5 `infrastructure/storage/local_asset_store.rs` and sidecar promotion
4. [ ] D4/D5 finish SQLite settings persistence and asset-store cache tests
5. [ ] D4/D5 finish storage schema parity tests

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
