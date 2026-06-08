#!/bin/bash
# sign-verification.sh — Called by the test-verification-architect agent after verifying a PR.
# Signs the PR's head commit SHA + ticket ID with the verifier's private key.
# Usage: scripts/sign-verification.sh <pr_number> <private_key_pem_string>

set -e

PR_NUMBER="$1"
PRIVATE_KEY="$2"

if [ -z "$PR_NUMBER" ] || [ -z "$PRIVATE_KEY" ]; then
    echo "ERROR: Usage: sign-verification.sh <pr_number> <private_key_pem_string>" >&2
    exit 1
fi

# Get the PR's current head SHA — this is what we're signing.
# If the PR gets new commits after signing, the signature will not match and merge is blocked.
HEAD_SHA=$(gh pr view "$PR_NUMBER" --json headRefOid -q .headRefOid 2>/dev/null || true)

if [ -z "$HEAD_SHA" ]; then
    echo "ERROR: Could not resolve PR #${PR_NUMBER} head SHA. Is the PR open?" >&2
    exit 1
fi

# Write private key to temp file
KEY_FILE=$(mktemp)
SHA_FILE=$(mktemp)
trap 'rm -f "$KEY_FILE" "$SHA_FILE"' EXIT
echo "$PRIVATE_KEY" > "$KEY_FILE"
echo -n "$HEAD_SHA" > "$SHA_FILE"

# Sign the SHA with Ed25519
SIGNATURE=$(openssl pkeyutl -sign -inkey "$KEY_FILE" -in "$SHA_FILE" | base64 -w 0)

if [ -z "$SIGNATURE" ]; then
    echo "ERROR: Signing failed — check private key format." >&2
    exit 1
fi

# Write approval file to the shared git common dir. This resolves to the main
# repo's .git/ even when called from a worktree (where .git is a pointer file,
# not a directory), so the wrapper at ~/.local/bin/gh can find it from either
# the worktree or the main repo.
GIT_COMMON_DIR=$(git rev-parse --path-format=absolute --git-common-dir 2>/dev/null)
if [ -z "$GIT_COMMON_DIR" ]; then
    echo "ERROR: Could not resolve git common dir. Are you in a git repository?" >&2
    exit 1
fi

mkdir -p "$GIT_COMMON_DIR/verification-approvals"
APPROVAL_FILE="$GIT_COMMON_DIR/verification-approvals/pr-${PR_NUMBER}"
cat > "$APPROVAL_FILE" <<EOF
${PR_NUMBER}
${HEAD_SHA}
${SIGNATURE}
EOF

echo "Verification approval signed for PR #${PR_NUMBER} at SHA ${HEAD_SHA}"
echo "Approval file: $APPROVAL_FILE"
