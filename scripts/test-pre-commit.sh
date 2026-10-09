#!/usr/bin/env bash
# Exercise hook outcomes with isolated Git content and a controlled Cargo command.
set -euo pipefail
project_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
fixture=$(mktemp -d)
trap 'rm -rf "$fixture"' EXIT
mkdir "$fixture/bin"
git -C "$fixture" init -q
printf 'fn main() {}\n' > "$fixture/main.rs"
git -C "$fixture" add main.rs
cat > "$fixture/bin/cargo" <<'CARGO'
#!/usr/bin/env bash
printf '%s\n' "${HOOK_TEST_OUTPUT:-}"
exit "${HOOK_TEST_EXIT:-0}"
CARGO
chmod +x "$fixture/bin/cargo"
export PATH="$fixture/bin:$PATH"
cd "$fixture"
hook="$project_dir/.githooks/pre-commit"
HOOK_TEST_OUTPUT='warning: the following packages contain code that will be rejected by a future version of Rust: dependency v1.0.0' bash "$hook" >/dev/null
if HOOK_TEST_OUTPUT='warning: unused variable: `value`' bash "$hook" >/dev/null; then
    echo 'ERROR: compiler warning accepted' >&2
    exit 1
fi
if HOOK_TEST_OUTPUT='warning: this expression creates a reference which is immediately dereferenced' bash "$hook" >/dev/null; then
    echo 'ERROR: Clippy warning accepted' >&2
    exit 1
fi
if HOOK_TEST_EXIT=101 HOOK_TEST_OUTPUT='error: compilation failed' bash "$hook" >/dev/null; then
    echo 'ERROR: Cargo failure accepted' >&2
    exit 1
fi
printf '#[allow(dead_code)]\nfn main() {}\n' > main.rs
git add main.rs
if bash "$hook" >/dev/null; then
    echo 'ERROR: lint suppression accepted' >&2
    exit 1
fi
echo 'Pre-commit fixtures passed.'
