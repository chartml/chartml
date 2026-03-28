#!/bin/bash
set -e
cd "$(dirname "$0")"
echo "Building WASM..."
wasm-pack build ../../crates/chartml-wasm --target web --out-dir ../../packages/core/pkg --out-name chartml
echo "Building TypeScript..."
npx tsc
echo "Done."
