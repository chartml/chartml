#!/bin/sh
# Always clean dist before building to prevent stale/missing CSS
rm -rf dist
trunk build --release "$@"
