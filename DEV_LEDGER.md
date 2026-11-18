# Development Ledger
**Last Updated:** 2026-11-19 00:35 | **Last Commit:** `HEAD` desktop: put Find over the rail in the top bar

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up. GUI work is allowed.
This session is working the next 10 commits; 8 are done. E5 shell parity is active.

## Next Steps (Ordered)
1. [ ] E5: match the Film Strip tile geometry and scroll track.
2. [ ] E5: replace reference-only panel values with backend services.
3. [ ] E6: start the dialogs over the new rail panels.

## Completed (Recent)
- `7eef57f5` desktop: put Find over the rail in the top bar.
- `a226baca` desktop: paint the grouped canvas bar.
- `04c2f5a9` desktop: name the canvas bar icon roles.
- `f5fc4827` desktop: space roll cards like the reference.
- `8a618a30` desktop: allocate the slider row it paints.
- `7eadf07c` desktop: keep section bodies inside the scroll.
- `38c8a7f6` desktop: capture golden frames at exact size.

## Verification
- Targeted `cargo test -p negpy-desktop --lib` passes per commit.
- Fresh 1920x1600 capture: Roll layout 0.633 after the top-bar move.
- `cargo check --workspace` and `cargo test --workspace` run after commit 10.
- `git diff --stat -- negpy negpy_core` must stay empty.

## Constraints / Gotchas
- Never edit `negpy/`; `negpy_core` stays its own workspace.
- Binding GUI reference: `negpy_egui/tests/gui/golden/tall/`.
- GUI parity is a layout score of 0.80, not pixel identity.
- Capture with `NEGPY_GUI_PROFILE=/home/bruno/Dokumenty/NegPy` and
  `NEGPY_GUI_FOLDER=/home/bruno/WORK/svetovy_vylet/skeny/vv9`.
- Generated captures live under `tests/gui/actual/` and stay untracked.
- The capture must be exactly 1920x1600 or an integer scale; never squash it.
- A collapsible body must advance the parent cursor by its measured height,
  or the next card overlaps it.
- A slider row must allocate only its own row, not the section rect.
- The 10-commit request supersedes the 5-commit checkpoint this session.
