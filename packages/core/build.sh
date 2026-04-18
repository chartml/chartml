#!/bin/bash
set -euo pipefail

# Dual-target WASM build for @chartml/core
# Produces web + nodejs JS glue sharing a single .wasm binary.

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
CRATE="crates/chartml-wasm"
OUT_DIR="packages/core/pkg"

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
mv "$OUT_DIR/web/chartml_wasm_bg.wasm" "$OUT_DIR/wasm/"
rm -f "$OUT_DIR/node/chartml_wasm_bg.wasm"

# Patch web JS to reference shared wasm
sed -i "s|new URL('chartml_wasm_bg.wasm', import.meta.url)|new URL('../wasm/chartml_wasm_bg.wasm', import.meta.url)|" "$OUT_DIR/web/chartml_wasm.js"

# Patch node JS to reference shared wasm
sed -i 's|`${__dirname}/chartml_wasm_bg.wasm`|`${__dirname}/../wasm/chartml_wasm_bg.wasm`|' "$OUT_DIR/node/chartml_wasm.js"

# Clean up generated package.json and gitignore from wasm-pack.
# We replace the auto-generated node `package.json` with a minimal one that
# pins `"type": "commonjs"` — wasm-pack's nodejs-target glue is CJS
# (`exports.WasmChartML = ...`), but the parent `@chartml/core/package.json`
# declares `"type": "module"`, which would otherwise make Node treat this
# `.js` as ESM and crash with `exports is not defined`.
rm -f "$OUT_DIR/web/package.json" "$OUT_DIR/web/.gitignore"
rm -f "$OUT_DIR/node/package.json" "$OUT_DIR/node/.gitignore"
cat > "$OUT_DIR/node/package.json" <<'EOF'
{
  "type": "commonjs"
}
EOF

# Build TypeScript wrapper
echo "Building TypeScript..."
npx tsc -p packages/core/tsconfig.json

echo "Built @chartml/core WASM package"
echo "  Web:  $OUT_DIR/web/"
echo "  Node: $OUT_DIR/node/"
echo "  WASM: $OUT_DIR/wasm/"
