#!/bin/bash
# verify-charts.sh — Verifies all golden SVGs have valid evaluator signatures.
# Used by pre-commit hook and CI to ensure no unapproved charts are committed.
#
# Usage: scripts/verify-charts.sh [golden_dir]
# Default golden_dir: test-output/golden
#
# Exit codes:
#   0 — all golden SVGs are signed and signatures match
#   1 — unsigned or invalid signatures found

set -e

GOLDEN_DIR="${1:-test-output/golden}"

# Chart evaluator public key (Ed25519)
PUBLIC_KEY="-----BEGIN PUBLIC KEY-----
MCowBQYDK2VwAyEA9+py+eA+gdXCxchd6IM+bA0ygA7OtOfk8/cHCnZ1lAo=
-----END PUBLIC KEY-----"

if [ ! -d "$GOLDEN_DIR" ]; then
    echo "Golden directory not found: $GOLDEN_DIR"
    exit 0  # No golden dir = nothing to verify
fi

# Find all golden SVGs
SVG_FILES=$(find "$GOLDEN_DIR" -name "*.svg" -not -name "*.sig" | sort)
TOTAL=0
UNSIGNED=0
INVALID=0
STALE=0
ERRORS=""

PUB_FILE=$(mktemp)
HASH_FILE=$(mktemp)
SIG_FILE=$(mktemp)
trap 'rm -f "$PUB_FILE" "$HASH_FILE" "$SIG_FILE"' EXIT
echo "$PUBLIC_KEY" > "$PUB_FILE"

for SVG in $SVG_FILES; do
    TOTAL=$((TOTAL + 1))
    SIG_PATH="${SVG}.sig"

    # Check signature file exists
    if [ ! -f "$SIG_PATH" ]; then
        UNSIGNED=$((UNSIGNED + 1))
        ERRORS="${ERRORS}\n  UNSIGNED: $SVG"
        continue
    fi

    # Read hash and signature from .sig file
    STORED_HASH=$(sed -n '1p' "$SIG_PATH")
    STORED_SIG=$(sed -n '2p' "$SIG_PATH")

    if [ -z "$STORED_HASH" ] || [ -z "$STORED_SIG" ]; then
        INVALID=$((INVALID + 1))
        ERRORS="${ERRORS}\n  MALFORMED SIG: $SIG_PATH"
        continue
    fi

    # Check SVG content matches stored hash
    CURRENT_HASH=$(sha256sum "$SVG" | awk '{print $1}')
    if [ "$CURRENT_HASH" != "$STORED_HASH" ]; then
        STALE=$((STALE + 1))
        ERRORS="${ERRORS}\n  STALE: $SVG (SVG changed since evaluator signed it)"
        continue
    fi

    # Verify Ed25519 signature
    echo -n "$STORED_HASH" > "$HASH_FILE"
    echo "$STORED_SIG" | base64 -d > "$SIG_FILE"

    if ! openssl pkeyutl -verify -pubin -inkey "$PUB_FILE" -rawin -in "$HASH_FILE" -sigfile "$SIG_FILE" >/dev/null 2>&1; then
        INVALID=$((INVALID + 1))
        ERRORS="${ERRORS}\n  INVALID SIGNATURE: $SVG"
        continue
    fi
done

# Report results
PASSED=$((TOTAL - UNSIGNED - INVALID - STALE))

if [ "$UNSIGNED" -eq 0 ] && [ "$INVALID" -eq 0 ] && [ "$STALE" -eq 0 ]; then
    echo "✅ All $TOTAL golden SVGs have valid evaluator signatures."
    exit 0
else
    echo "❌ Chart signature verification failed."
    echo ""
    echo "  Total:    $TOTAL"
    echo "  Passed:   $PASSED"
    echo "  Unsigned: $UNSIGNED"
    echo "  Stale:    $STALE"
    echo "  Invalid:  $INVALID"
    echo ""
    echo "  Failures:"
    echo -e "$ERRORS"
    echo ""
    echo "  Run the chart-evaluator agent to review and sign unapproved charts."
    echo "  Only the chart-evaluator agent can produce valid signatures."
    exit 1
fi
