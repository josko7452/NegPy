# NegFe

The Rust desktop shell over `negpy-backend`.

## Run

```sh
cd negpy_egui
cargo run --release --bin negfe -- \
  --profile ~/Dokumenty/NegPy \
  --folder /path/to/roll
```

`NEGPY_GUI_PROFILE` and `NEGPY_GUI_FOLDER` set the same values and take
priority over the flags. A missing folder shows the placeholder canvas. The
first image of the folder is not decoded; set `NEGPY_GUI_FOLDER` to a folder
that holds the reference frame.

## Capture

```sh
cd negpy_egui
cargo run --release --bin negfe-capture -- \
  --golden tests/gui/golden/tall
```

The capture binary writes the golden matrix to `tests/gui/actual/rust/`.
