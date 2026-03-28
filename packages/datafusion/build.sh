#!/bin/bash
set -e
cd "$(dirname "$0")"
echo "Building WASM..."
wasm-pack build ../../crates/chartml-wasm-datafusion --target web --out-dir ../../packages/datafusion/pkg --out-name chartml-datafusion
echo "Building TypeScript..."
npx tsc
echo "Done."
