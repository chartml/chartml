#!/bin/bash
# sign-chart.sh — Signs a golden SVG with the chart evaluator's Ed25519 private key.
# Called by the chart-evaluator agent after approving a chart.
#
# Usage: scripts/sign-chart.sh <private_key_pem> <svg_path>
# Example: scripts/sign-chart.sh "$KEY" test-output/golden/bar/basic_3_months.svg
#
# Creates <svg_path>.sig containing:
#   Line 1: SHA256 hash of SVG content
#   Line 2: Base64-encoded Ed25519 signature of the hash

set -e

PRIVATE_KEY="$1"
SVG_PATH="$2"

if [ -z "$PRIVATE_KEY" ]; then
    echo "ERROR: Private key argument required." >&2
    exit 1
fi

if [ -z "$SVG_PATH" ] || [ ! -f "$SVG_PATH" ]; then
    echo "ERROR: SVG file not found: $SVG_PATH" >&2
    exit 1
fi

# Write private key to temp file
KEY_FILE=$(mktemp)
HASH_FILE=$(mktemp)
trap 'rm -f "$KEY_FILE" "$HASH_FILE"' EXIT
echo "$PRIVATE_KEY" > "$KEY_FILE"

# Compute SHA256 of SVG content
SVG_HASH=$(sha256sum "$SVG_PATH" | awk '{print $1}')

# Sign the hash with Ed25519 (must use -in file, not stdin)
echo -n "$SVG_HASH" > "$HASH_FILE"
SIGNATURE=$(openssl pkeyutl -sign -inkey "$KEY_FILE" -in "$HASH_FILE" | base64 -w 0)

if [ -z "$SIGNATURE" ]; then
    echo "ERROR: Signing failed — check private key format." >&2
    exit 1
fi

# Write signature file alongside the SVG
SIG_PATH="${SVG_PATH}.sig"
cat > "$SIG_PATH" <<EOF
${SVG_HASH}
${SIGNATURE}
EOF

echo "Signed: $SVG_PATH (hash: ${SVG_HASH:0:16}...)"
