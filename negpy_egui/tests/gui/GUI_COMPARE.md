# GUI Comparison

The Python app is the reference. A screenshot of it defines the layout and the
look that NegFe must keep. A comparison is not a pixel test: the two toolkits
render text and icons differently, and a pixel test would fail on a font hint
alone.

## Capture

Both apps write one PNG at a fixed logical size. The capture is the window
content, with no desktop background, cursor or window decoration.

- Python: `QT_QPA_PLATFORM=offscreen` and `QWidget.grab()`. A golden test sets
  `tutorial_seen` in its isolated user directory, sets the window size, opens
  the fixture roll, waits for the frame, and saves the image.
- Rust: `ViewportCommand::Screenshot` with `egui::UserData::default()`.
  `App::update` reads `egui::Event::Screenshot` and writes its `ColorImage` to
  PNG. `run_headless` supplies the same fixed viewport and fixture state.

The golden logical size is 1920x1600. It shows the Roll and Exposure card stacks
without scrolling. A HiDPI capture is divided by its scale factor before it
reaches the comparator.

The capture writes the client area, not the window frame. A test asserts the
image size before it compares, because a client area one pixel short changes
every region ratio.

## Fixture

`reference_roll.json` names the state both apps must show:

- the first frame of the reference roll
- the Roll tab, with Setup, Raw Decode and Optics visible
- the Analysis card open, Log scale on
- the Film Strip open at its default height
- the canvas fitted to the frame

The capture uses the real source file. The comparator scores the frame area by
placement, not by pixels, so a different grade does not hide a layout fault.

## Golden set

`golden/tall/` is the binding reference set. It holds 19 window captures at
1920x1600 and one Contact Sheet dialog at 1080x760. It covers every top tab,
every Metadata and Export sub-tab, the Light Table and the Contact Sheet
dialog. `golden/tall/manifest.json` records the profile, frame, tab, sub-tab and
view of every image.

```sh
.venv/bin/python negpy_egui/tests/gui/capture_matrix.py \
  --profile ~/Dokumenty/NegPy \
  --folder /path/to/roll \
  --frame P5080635.ORF \
  --output-dir /tmp/negpy-matrix
```

The harness copies the profile, suppresses the restore prompt, restores the
camera rig, loads the frame once, then captures the tab and sub-tab matrix. It
also opens the Contact Sheet dialog and saves it after its tiles render.

The first Rust GUI task is the same capture path. `desktop/src/capture.rs` uses
`ViewportCommand::Screenshot` and writes `egui::Event::Screenshot.image` to PNG.
`desktop/src/bin/negfe-capture.rs` reproduces the golden manifest. The Rust app
is not complete on that first run; the capture binary must still write every
PNG and a manifest before the comparison starts.

## Regions

Coordinates are fractions of the window. This keeps one fixture valid at any
window size.

| Region | Box (x0, y0, x1, y1) | What it must show |
|---|---|---|
| `top_bar` | 0.000, 0.000, 1.000, 0.033 | File name left, recipe right, Find field in the rail |
| `canvas` | 0.004, 0.033, 0.725, 0.806 | Gray background with the fitted frame |
| `rail` | 0.728, 0.033, 0.996, 0.995 | Tab row, Analysis chart, Roll cards |
| `strip` | 0.004, 0.808, 0.725, 0.962 | Film Strip frames |
| `toolbar` | 0.004, 0.963, 0.725, 0.995 | Photo, navigation, Print, Roll and the view menu |

The exact box for each control is read from the Python widget tree, not typed
from the screenshot. `reference_roll.json` holds the boxes that the first
comparison confirmed.

## Metrics

Each region reports the four values below. The total score is the weighted sum.

- `geometry`: correlation of the region's row and column brightness profiles.
  This catches a rail that moved, a strip that collapsed, or a toolbar of the
  wrong height, while ignoring glyph antialiasing.
- `palette`: mean CIEDE2000 distance between the eight most common colors of
  each region. This catches a surface or border token that drifted.
- `structure`: gradient-magnitude correlation after a 1.5 px blur. This catches
  a missing card, a control that lost its border, or a tab row that vanished,
  while ignoring glyph shape.
- `text`: OCR word boxes compared by position and count. Only the region's
  labels are compared, not their wording, because translation changes the text.

The canvas region compares the photo rectangle by intersection over union, so a
different grade or a different photo does not count.

The thresholds in `reference_roll.json` are the budget. The gate is the layout
score: **0.80 is the target**, not 1.00. A region below its own floor prints the
two images that differ, but only the layout score decides pass or fail. Pixel
identity is not the goal; the same controls, order, grouping and hierarchy are.

## Commands

```sh
python3 negpy_egui/tests/gui/compare.py \
  --reference /path/to/reference.png \
  --candidate negpy_egui/tests/gui/actual/negfe.png \
  --report negpy_egui/tests/gui/actual/report.html
```

The report is one row per region, with the score and the cropped images. It is
a review aid, not a committed file.
