# Golden set

This directory is the binding GUI reference for the Rust port. It was captured
from the Python app on 2026-11-17 with `capture_matrix.py`, using a copied
NegPy profile, the reference roll folder and the frame `P5080635.ORF`.

The tall set stays the Python reference. `../narrow/` is the tracked Rust guard
at 1400x900. A fresh Rust tall capture is review evidence only.

## Contents

- 19 window captures at 1920x1600: every top tab, every Metadata and Export
  sub-tab, the Light Table and the Scan tab.
- One Contact Sheet dialog at 1080x760 with its tiles rendered.
- `manifest.json`: the profile, frame, tab, sub-tab and view of every image, and
  the source profile and folder that regenerate it.
- `contact_sheet.png`: a review contact sheet. It is not a comparison target.

## Rules

- A GUI change is compared against the matching image before it merges.
- The gate is the layout score from `../compare.py`, 0.80 or better. Pixel
  identity is not the goal.
- Add a golden only with its manifest entry. Do not overwrite one without the
  same profile, frame, tab and view.
- Rebuild with:

```sh
.venv/bin/python negpy_egui/tests/gui/capture_matrix.py \
  --profile ~/Dokumenty/NegPy \
  --folder /home/bruno/WORK/svetovy_vylet/skeny/vv9 \
  --frame P5080635.ORF \
  --output-dir /tmp/negpy-golden \
  --height 1600 --settle 3 --timeout 180
```

The script copies the profile, suppresses the restore prompt, sets the camera
rig and white light, and saves the tab and sub-tab matrix plus the dialog.
