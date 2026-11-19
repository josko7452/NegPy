# Development Ledger
**Last Updated:** 2026-11-19 10:05 | **Last Commit:** `HEAD` desktop: card the Library roll list

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up. GUI work is allowed.
Ten commits are complete this session; E5 shell parity is active.

## Next Steps (Ordered)
1. [ ] E5: wire History to the session's real work prints and steps.
2. [ ] E5: give Geometry its taller toggle chips.
3. [ ] E5: tighten the Roll rail cards.

## Completed (Recent)
- `a6a609df` desktop: card the Library roll list.
- `2072aaf7` desktop: build the Frame History page.
- `78ab72cf` desktop: align the Geometry crop rows.
- `1681c90f` desktop: align the Metadata shot rows.
- `a9b8e0e6` desktop: open Scan on the camera rig.
- `fd019cf9` desktop: card the Export file form.
- `0f8ca17c` desktop: fill the top bar status line.
- `335984e5` desktop: group the canvas toolbar like the reference.
- `056977a6` desktop: show the roll in the Film Strip.
- `5a364f3a` desktop: open the Frame capture sub-tab.

## Verification
- `cargo test --workspace` passes. `cargo check --workspace` passes.
  `cargo fmt --all --check` passes.
- Clippy is blocked by existing lints in `negpy-backend` and older
  desktop modules, such as `manual_is_multiple_of` and
  `too_many_arguments`. None come from this session's new modules.
- Full golden comparison: 20 captures, 0.660 to 0.783. Frame Favorites
  is at 0.783, the highest. Light Table and Scan are at 0.66 to 0.67.
- The Frame sub-tab capture bug is fixed. All six Frame pages open.

## Constraints / Gotchas
- Never edit `negpy/`; `negpy_core` stays its own workspace.
- Binding GUI reference: `negpy_egui/tests/gui/golden/tall/`.
- GUI parity is a layout score of 0.80, not pixel identity.
- Capture with `NEGPY_GUI_PROFILE=/home/bruno/Dokumenty/NegPy` and
  `NEGPY_GUI_FOLDER=/home/bruno/WORK/svetovy_vylet/skeny/vv9`.
- Generated captures live under `tests/gui/actual/` and stay untracked.
- Thumbnails come from the profile cache. A cache miss must not run a
  full RAW demosaic in the capture harness.
- The capture spends most of its time encoding PNGs; allow about two
  minutes and score the frames that saved.
- The 10-commit request supersedes the 5-commit checkpoint this session.
