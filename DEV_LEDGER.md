# Development Ledger
**Last Updated:** 2026-11-19 09:31 | **Last Commit:** `HEAD` desktop: build the Frame History page

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up. GUI work is allowed.
Ten-commit session in progress; E5 shell parity is active.

## Next Steps (Ordered)
1. [ ] E5: wire History to the session's real work prints and steps.
2. [ ] E5: tighten the Library rows and toolbar.
3. [ ] E5: reproduce frame captures after the rail fixes.

## Completed (Recent)
- `HEAD` desktop: build the Frame History page.
- `78ab72cf` desktop: align the Geometry crop rows.
- `1681c90f` desktop: align the Metadata shot rows.

## Verification
- Targeted `cargo test -p negpy-desktop --lib` passes per commit.
- Frame History layout rose from 0.720 to 0.761. The rail layout is
  0.793, and the page now paints Work Prints and Edit History.
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
