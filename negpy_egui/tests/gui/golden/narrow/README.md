# Narrow guard set

These captures come from the Rust app at 1400x900. They show the shell at the
smallest supported window, where the rail and Film Strip lose their full width.

The tall set stays the Python reference. A layout change compares against this
set only for the narrow window, so a fault that appears after the window shrinks
still fails.

The captures come from the linear source, so the tone and Analysis cards show
the rendered histogram. The layout gate is 0.80.

Regenerate them with the empty fixture in `manifest.json`:

```sh
cd negpy_egui
cargo run --release --bin negfe-capture -- \
  --golden tests/gui/golden/narrow \
  --output tests/gui/golden/narrow
```

The capture binary needs the profile and folder that produced the last set.
`manifest.json` records both in `source_profile` and `source_folder`.
