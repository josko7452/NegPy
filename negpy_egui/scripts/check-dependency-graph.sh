#!/bin/sh
# Keep the headless backend free of GUI and Python crates, and the GUI crate free
# of Python crates. CI and the Makefile share this gate.
set -eu

cd "$(dirname "$0")/.."

if cargo tree -p negpy-backend --prefix none | grep -qE '^(pyo3|numpy|eframe|egui)'; then
    echo "negpy-backend pulls in a GUI or Python crate" >&2
    exit 1
fi

if cargo tree -p negpy-desktop --prefix none | grep -qE '^(pyo3|numpy)'; then
    echo "negpy-desktop pulls in a Python crate" >&2
    exit 1
fi
