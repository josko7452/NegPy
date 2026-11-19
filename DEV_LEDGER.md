# Development Ledger
**Last Updated:** 2026-11-19 07:58 | **Last Commit:** `HEAD` desktop: fill the top bar status line

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up. GUI work is allowed.
Ten-commit session in progress; E5 shell parity is active.

## Next Steps (Ordered)
1. [ ] E5: bring the Export rail to the reference structure.
2. [ ] E5: bring the Scan rail to the reference structure.
3. [ ] E5: tighten the Metadata rail cards.

## Completed (Recent)
- `HEAD` desktop: fill the top bar status line.
- `335984e5` desktop: group the canvas toolbar like the reference.
- `056977a6` desktop: show the roll in the Film Strip.

## Verification
- Targeted `cargo test -p negpy-desktop --lib` passes per commit.
- Top bar passes its region thresholds. Layout 0.608, geometry 0.758.
- Full workspace tests run after commit 10.

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
