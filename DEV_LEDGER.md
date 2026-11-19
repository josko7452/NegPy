# Development Ledger
**Last Updated:** 2026-11-19 08:54 | **Last Commit:** `HEAD` desktop: align the Metadata shot rows

## Current Goal
Port `negpy/` to Rust in `negpy_egui/`, bottom-up. GUI work is allowed.
Ten-commit session in progress; E5 shell parity is active.

## Next Steps (Ordered)
1. [ ] E5: match the Frame geometry card rows.
2. [ ] E5: tighten the Library and Frame History rails.
3. [ ] E5: reproduce frame captures after the rail fixes.

## Completed (Recent)
- `HEAD` desktop: align the Metadata shot rows.
- `a9b8e0e6` desktop: open Scan on the camera rig.
- `fd019cf9` desktop: card the Export file form.

## Verification
- Targeted `cargo test -p negpy-desktop --lib` passes per commit.
- Metadata Shot layout rose from 0.652 to 0.687. Rail geometry rose
  from 0.553 to 0.594.
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
