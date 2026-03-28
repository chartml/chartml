#!/bin/bash
set -euo pipefail

# Dual-target WASM build for @chartml/datafusion
# Produces web + nodejs JS glue sharing a single .wasm binary.

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
CRATE="crates/chartml-wasm-datafusion"
OUT_DIR="packages/datafusion/pkg"

cd "$REPO_ROOT"

# Clean
rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR/web" "$OUT_DIR/node" "$OUT_DIR/wasm"

# Build both targets
echo "Building WASM (web target)..."
wasm-pack build "$CRATE" --target web --out-dir "../../$OUT_DIR/web"

echo "Building WASM (nodejs target)..."
wasm-pack build "$CRATE" --target nodejs --out-dir "../../$OUT_DIR/node"

# Move wasm to shared location (both targets produce identical .wasm)
mv "$OUT_DIR/web/chartml_wasm_datafusion_bg.wasm" "$OUT_DIR/wasm/"
rm -f "$OUT_DIR/node/chartml_wasm_datafusion_bg.wasm"

# Patch web JS to reference shared wasm
sed -i "s|new URL('chartml_wasm_datafusion_bg.wasm', import.meta.url)|new URL('../wasm/chartml_wasm_datafusion_bg.wasm', import.meta.url)|" "$OUT_DIR/web/chartml_wasm_datafusion.js"

# Patch node JS to reference shared wasm
sed -i 's|`${__dirname}/chartml_wasm_datafusion_bg.wasm`|`${__dirname}/../wasm/chartml_wasm_datafusion_bg.wasm`|' "$OUT_DIR/node/chartml_wasm_datafusion.js"

# Clean up generated package.json and gitignore from wasm-pack
rm -f "$OUT_DIR/web/package.json" "$OUT_DIR/web/.gitignore"
rm -f "$OUT_DIR/node/package.json" "$OUT_DIR/node/.gitignore"

# Build TypeScript wrapper
echo "Building TypeScript..."
npx tsc -p packages/datafusion/tsconfig.json

echo "Built @chartml/datafusion WASM package"
echo "  Web:  $OUT_DIR/web/"
echo "  Node: $OUT_DIR/node/"
echo "  WASM: $OUT_DIR/wasm/"
