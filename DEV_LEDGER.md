# Development Ledger
**Last Updated:** 2026-11-19 07:25 | **Last Commit:** `HEAD` desktop: show the roll in the Film Strip

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up. GUI work is allowed.
Ten-commit session in progress; E5 shell parity is active.

## Next Steps (Ordered)
1. [ ] E5: match the canvas toolbar groups and widths.
2. [ ] E5: match the top bar copy and spacing.
3. [ ] E5: bring the Export rail to the reference structure.

## Completed (Recent)
- `HEAD` desktop: show the roll in the Film Strip.
- `5a364f3a` desktop: open the Frame capture sub-tab.
- `3ea357f8` desktop: match the Film Strip tile track.

## Verification
- Targeted `cargo test -p negpy-desktop --lib` passes per commit.
- Frame captures open their manifest page. Strip geometry rose from
  0.366 to 0.648 and layout from 0.111 to 0.319 on Roll Top.
- Full workspace tests run after commit 10.

## Constraints / Gotchas
- Never edit `negpy/`; `negpy_core` stays its own workspace.
- Binding GUI reference: `negpy_egui/tests/gui/golden/tall/`.
- GUI parity is a layout score of 0.80, not pixel identity.
- Capture with `NEGPY_GUI_PROFILE=/home/bruno/Dokumenty/NegPy` and
  `NEGPY_GUI_FOLDER=/home/bruno/WORK/svetovy_vylet/skeny/vv9`.
- Generated captures live under `tests/gui/actual/` and stay untracked.
- The capture must be exactly 1920x1600 or an integer scale; never squash it.
- Thumbnails come from the profile cache. A cache miss must not run a
  full RAW demosaic in the capture harness.
- The capture takes about two minutes; run it with a timeout and score
  the frames that saved.
- The 10-commit request supersedes the 5-commit checkpoint this session.
