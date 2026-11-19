# Development Ledger
**Last Updated:** 2026-11-19 03:10 | **Last Commit:** `HEAD` desktop: open the Frame capture sub-tab

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up. GUI work is allowed.
Ten-commit session in progress; E5 shell parity is active.

## Next Steps (Ordered)
1. [ ] E5: measure the rail regions after the Frame capture fix.
2. [ ] E5: bring the toolbar and rail card interiors to the golden layout.
3. [ ] E5: replace reference-only panel values with backend services.

## Completed (Recent)
- `HEAD` desktop: open the Frame capture sub-tab.
- `3ea357f8` desktop: match the Film Strip tile track.
- `c8191452` desktop: show the card header chrome.

## Verification
- Targeted `cargo test -p negpy-desktop --lib` passes per commit.
- Frame captures now open Geometry, Exposure, Look, Finish, Favorites
  and History from the manifest's `tab` key.
- Full workspace tests run after commit 10.

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
