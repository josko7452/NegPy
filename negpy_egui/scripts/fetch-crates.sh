#!/usr/bin/env bash
# Populate the shared Cargo registry cache with every crate negpy_egui pins.
#
# The development sandbox has no network and a read-only ~/.cargo, so a build there can
# only use crates already present in the cache. Run this script from a networked shell
# after Cargo.toml or Cargo.lock changes; it downloads the pinned crates into the shared
# ~/.cargo/registry, where an offline `cargo build --offline` then finds them.
#
# Usage: negpy_egui/scripts/fetch-crates.sh
set -euo pipefail

cd "$(dirname "$0")/.."

echo "Fetching negpy_egui pinned crates into ~/.cargo/registry ..."
cargo fetch --locked

echo "Done. Offline builds in the sandbox can now resolve every pinned crate."
