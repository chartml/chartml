#!/bin/bash
# chart-status.sh — Full status report for chart validation.
# Combines regression check (current vs golden) with signature verification.
#
# Usage: scripts/chart-status.sh
#
# Reports:
#   - Charts that changed (current output differs from golden)
#   - Charts with no evaluator signature
#   - Charts with stale signatures (golden SVG changed after signing)
#   - Charts with invalid signatures (tampered or wrong key)
#   - New charts (in current output but no golden baseline)
#   - Summary with action items

CURRENT_DIR="test-output/all"
GOLDEN_DIR="test-output/golden"

# Chart evaluator public key
PUBLIC_KEY="-----BEGIN PUBLIC KEY-----
MCowBQYDK2VwAyEAFBoUllCvCjUKbZy+ZVDwpC6GVGBMHU43nlaIeS8zus0=
-----END PUBLIC KEY-----"

PUB_FILE=$(mktemp)
HASH_FILE=$(mktemp)
SIG_BIN=$(mktemp)
trap 'rm -f "$PUB_FILE" "$HASH_FILE" "$SIG_BIN"' EXIT
echo "$PUBLIC_KEY" > "$PUB_FILE"

# Counters
TOTAL_GOLDEN=0
SIGNED_VALID=0
UNSIGNED=0
STALE_SIG=0
INVALID_SIG=0
CHANGED=0
NEW_CHARTS=0

# Lists for reporting
UNSIGNED_LIST=""
STALE_LIST=""
INVALID_LIST=""
CHANGED_LIST=""
NEW_LIST=""

if [ ! -d "$GOLDEN_DIR" ]; then
    echo "No golden baseline directory. Run --batch then --accept first."
    exit 1
fi

# Check each golden SVG
for SVG in $(find "$GOLDEN_DIR" -name "*.svg" -not -name "*.sig" | sort); do
    REL=$(echo "$SVG" | sed "s|^$GOLDEN_DIR/||")
    NAME=$(echo "$REL" | sed 's/\.svg$//')
    TOTAL_GOLDEN=$((TOTAL_GOLDEN + 1))

    SIG_PATH="${SVG}.sig"

    # Check signature
    if [ ! -f "$SIG_PATH" ]; then
        UNSIGNED=$((UNSIGNED + 1))
        UNSIGNED_LIST="${UNSIGNED_LIST}\n  ${NAME}"
    else
        STORED_HASH=$(sed -n '1p' "$SIG_PATH")
        STORED_SIG=$(sed -n '2p' "$SIG_PATH")
        CURRENT_HASH=$(sha256sum "$SVG" | awk '{print $1}')

        if [ -z "$STORED_HASH" ] || [ -z "$STORED_SIG" ]; then
            INVALID_SIG=$((INVALID_SIG + 1))
            INVALID_LIST="${INVALID_LIST}\n  ${NAME} (malformed .sig file)"
        elif [ "$CURRENT_HASH" != "$STORED_HASH" ]; then
            STALE_SIG=$((STALE_SIG + 1))
            STALE_LIST="${STALE_LIST}\n  ${NAME}"
        else
            echo -n "$STORED_HASH" > "$HASH_FILE"
            echo "$STORED_SIG" | base64 -d > "$SIG_BIN" 2>/dev/null
            if openssl pkeyutl -verify -pubin -inkey "$PUB_FILE" -in "$HASH_FILE" -sigfile "$SIG_BIN" >/dev/null 2>&1; then
                SIGNED_VALID=$((SIGNED_VALID + 1))
            else
                INVALID_SIG=$((INVALID_SIG + 1))
                INVALID_LIST="${INVALID_LIST}\n  ${NAME} (signature mismatch)"
            fi
        fi
    fi

    # Check for regression (current output differs from golden)
    CURRENT_SVG="$CURRENT_DIR/$REL"
    if [ -f "$CURRENT_SVG" ]; then
        if ! diff -q "$SVG" "$CURRENT_SVG" >/dev/null 2>&1; then
            CHANGED=$((CHANGED + 1))
            CHANGED_LIST="${CHANGED_LIST}\n  ${NAME}"
        fi
    fi
done

# Check for new charts (in current but not golden)
if [ -d "$CURRENT_DIR" ]; then
    for SVG in $(find "$CURRENT_DIR" -name "*.svg" | sort); do
        REL=$(echo "$SVG" | sed "s|^$CURRENT_DIR/||")
        GOLDEN_SVG="$GOLDEN_DIR/$REL"
        if [ ! -f "$GOLDEN_SVG" ]; then
            NEW_CHARTS=$((NEW_CHARTS + 1))
            NAME=$(echo "$REL" | sed 's/\.svg$//')
            NEW_LIST="${NEW_LIST}\n  ${NAME}"
        fi
    done
fi

# Report
NEEDS_ACTION=$((UNSIGNED + STALE_SIG + INVALID_SIG + CHANGED + NEW_CHARTS))

echo "=== Chart Status ==="
echo ""
echo "  Golden SVGs:      $TOTAL_GOLDEN"
echo "  Signed & valid:   $SIGNED_VALID"
echo "  Unsigned:         $UNSIGNED"
echo "  Stale signature:  $STALE_SIG"
echo "  Invalid signature: $INVALID_SIG"
echo "  Changed (regressed): $CHANGED"
echo "  New (no golden):  $NEW_CHARTS"
echo ""

if [ "$NEEDS_ACTION" -eq 0 ]; then
    echo "✅ All $TOTAL_GOLDEN charts are clean — signed, valid, and matching current output."
    exit 0
fi

echo "❌ $NEEDS_ACTION chart(s) need attention:"

if [ -n "$CHANGED_LIST" ]; then
    echo ""
    echo "  REGRESSED (current output differs from golden — re-render, re-evaluate, re-sign):"
    echo -e "$CHANGED_LIST"
fi

if [ -n "$UNSIGNED_LIST" ]; then
    echo ""
    echo "  UNSIGNED (need chart-evaluator to evaluate and sign):"
    echo -e "$UNSIGNED_LIST"
fi

if [ -n "$STALE_LIST" ]; then
    echo ""
    echo "  STALE (golden SVG changed after evaluator signed — re-evaluate and re-sign):"
    echo -e "$STALE_LIST"
fi

if [ -n "$INVALID_LIST" ]; then
    echo ""
    echo "  INVALID (signature doesn't verify — re-evaluate and re-sign):"
    echo -e "$INVALID_LIST"
fi

if [ -n "$NEW_LIST" ]; then
    echo ""
    echo "  NEW (run --accept to promote to golden, then evaluate and sign):"
    echo -e "$NEW_LIST"
fi

echo ""
echo "Run the chart-evaluator agent to resolve these."
exit 1
