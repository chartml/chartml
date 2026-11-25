#!/usr/bin/env bash
# CHA-35: prove what @chartml/core publishes cannot drift from source.
#  1. No build output (packages/*/dist, packages/*/pkg) is tracked in git.
#  2. The packed tarball's "./style.css" export resolves to a file that is
#     byte-identical to packages/core/src/chartml.css.
#  3. The tarball contains the built dist/index.js and dist/index.d.ts
#     (run after `bash packages/core/build.sh`).
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"

tracked=$(git ls-files 'packages/*/dist/*' 'packages/*/pkg/*')
if [ -n "$tracked" ]; then
  echo "::error::Build output is committed; it is built from source and must not be tracked:"
  echo "$tracked"
  exit 1
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
npm pack --workspace packages/core --pack-destination "$work" --silent >/dev/null
tarball=$(ls "$work"/*.tgz)
tar -xzf "$tarball" -C "$work"

target=$(node -e 'const p=JSON.parse(require("fs").readFileSync(process.argv[1]));process.stdout.write(p.exports["./style.css"])' "$work/package/package.json")
if [ ! -f "$work/package/$target" ]; then
  echo "::error::Packed tarball is missing the style.css export target: $target"
  exit 1
fi
if ! cmp -s "$work/package/$target" packages/core/src/chartml.css; then
  echo "::error::Packed style.css ($target) differs from packages/core/src/chartml.css"
  exit 1
fi

for f in dist/index.js dist/index.d.ts; do
  if [ ! -s "$work/package/$f" ]; then
    echo "::error::Packed tarball is missing built file $f (run packages/core/build.sh first)"
    exit 1
  fi
done
echo "OK: packed style.css ($target) matches src/chartml.css; built dist present; no tracked build output."
