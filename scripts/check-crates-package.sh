#!/usr/bin/env bash
# CHA-58: prove every crate builds against its sibling crates as they will be
# published to crates.io.
#
# The workspace compiles its crates against each other by path, which hides a
# breaking change in chartml-core: a chart crate published earlier still asks
# crates.io for `chartml-core = "5"` and would be paired with a core that no
# longer compiles it. `cargo package` removes the path and keeps the version
# requirement, then builds the packaged crate. Packaging all the crates in one
# run makes cargo resolve each version requirement against the other crates in
# this repository (not against what crates.io holds), in dependency order, so a
# requirement the new core does not satisfy, or a crate that no longer compiles
# against it, fails here before anything is published.
#
# Needs cargo 1.90 or newer (multi-package `cargo package`) and a clean git
# tree: commit the version bumps first. The crate list is the one
# publish-crates.yml publishes, in its order.
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"

cargo package \
  -p chartml-core \
  -p chartml-forecast \
  -p chartml-chart-cartesian \
  -p chartml-chart-pie \
  -p chartml-chart-scatter \
  -p chartml-chart-metric \
  -p chartml-chart-table \
  -p chartml-datafusion \
  -p chartml-render \
  -p chartml-leptos
