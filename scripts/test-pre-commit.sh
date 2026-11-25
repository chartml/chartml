#!/usr/bin/env bash
# Exercise .githooks/pre-commit in throwaway Git repositories.
#
# The hook must judge only the commit's content: staged lint suppressions and
# clippy warnings fail, while unstaged and untracked work must survive every
# outcome (pass, fail, interrupt) completely unchanged.
set -euo pipefail

project_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
hook="$project_dir/.githooks/pre-commit"
scratch=$(mktemp -d)
# The hook keeps its lint copy outside the repository, so the suite must remove
# the copies it caused; otherwise every run leaves one behind per fixture.
cleanup_scratch() {
    local pointer copy found
    set +f 2>/dev/null || true
    while IFS= read -r pointer; do
        if [ -z "$pointer" ] || [ ! -f "$pointer" ]; then
            continue
        fi
        copy=$(cat -- "$pointer" 2>/dev/null) || copy=''
        case "$copy" in
            */chartml-pre-commit.*) rm -rf -- "$copy" ;;
        esac
    done <<EOF
$(find "$scratch" -name snapshot-dir -path '*chartml-pre-commit/snapshot-dir' 2>/dev/null || true)
EOF
    rm -rf -- "$scratch"
}
trap cleanup_scratch EXIT

failures=0
pass() { echo "PASS: $*"; }
fail() {
    echo "FAIL: $*" >&2
    failures=$((failures + 1))
}

# The hook keeps its lint copy outside the repository, so tests read its
# location from the pointer the hook writes.
snapshot_dir() {
    local repo=$1 git_dir pointer root
    git_dir=$(git -C "$repo" rev-parse --absolute-git-dir) || return 1
    pointer="$git_dir/chartml-pre-commit/snapshot-dir"
    if [ ! -f "$pointer" ]; then
        return 1
    fi
    root=$(cat -- "$pointer")
    printf '%s/tree' "$root"
}

# --- Fixture shared by every scenario -------------------------------------
# A fake cargo on PATH records the directory it linted, so the tests can prove
# the hook lints a copy outside the working tree, and can simulate warnings,
# failures and hangs. It also fails if the copy it is given sits inside the
# repository, which is how unstaged files used to leak into the verdict.
setup_bin() {
    mkdir -p "$scratch/bin"
    cat > "$scratch/bin/cargo" <<'CARGO'
#!/usr/bin/env bash
printf '%s\n' "$PWD" >> "${HOOK_TEST_PWD_LOG:-/dev/null}"
if [ -n "${HOOK_TEST_PID_FILE:-}" ]; then
    printf '%s\n' "$$" > "$HOOK_TEST_PID_FILE"
fi
# A lint copy inside the working tree lets Cargo find the working tree's
# manifest and .cargo/config.toml, so unstaged files would change the verdict.
if [ -n "${HOOK_TEST_REPO_ROOT:-}" ]; then
    case "$PWD/" in
        "$HOOK_TEST_REPO_ROOT"/*)
            printf 'warning: lint copy is inside the working tree: %s\n' "$PWD"
            ;;
    esac
fi
printf '%s\n' "${HOOK_TEST_OUTPUT:-}"
# Emulate Cargo's own config discovery: a .cargo/config.toml in the linted
# directory (or an ancestor) with rustflags that silence warnings really does
# suppress them. Without this the isolation test would pass no matter where the
# copy sat, so it could not catch the defect it exists for.
suppress=0
# Cargo honours build.rustflags from config files it discovers, but RUSTFLAGS
# in the environment overrides them entirely — even an empty value. Emulate
# that, so the isolation tests fail when a setting outside the commit reaches
# the linted content.
if [ -n "${RUSTFLAGS+set}" ]; then
    case "${RUSTFLAGS:-}" in
        *-A*warnings*|*allow*) suppress=1 ;;
    esac
fi
if [ -n "${RUSTC_WRAPPER:-}${RUSTC_WORKSPACE_WRAPPER:-}" ] && grep -qE -- '-A"? ?"?warnings' "${RUSTC_WRAPPER:-${RUSTC_WORKSPACE_WRAPPER:-}}" 2>/dev/null; then
    suppress=1
fi
if [ "$suppress" -eq 0 ] && [ -z "${RUSTFLAGS+set}" ]; then
    check_dir=$PWD
    while :; do
        if [ -f "$check_dir/.cargo/config.toml" ] && grep -qE -- '-A"?,? ?"warnings|allow' "$check_dir/.cargo/config.toml" 2>/dev/null; then
            suppress=1
            break
        fi
        [ "$check_dir" = "/" ] && break
        check_dir=$(dirname -- "$check_dir")
    done
fi
# Emit a warning when the linted content itself contains a marker, so tests
# can prove the verdict follows the linted content and nothing else.
if [ -n "${HOOK_TEST_WARN_ON_MARKER:-}" ] && [ "$suppress" -eq 0 ] && grep -rqF -- "$HOOK_TEST_WARN_ON_MARKER" .; then
    printf 'warning: unused variable: `marker`\n'
fi
if [ -n "${HOOK_TEST_HANG_MARKER:-}" ]; then
    : > "$HOOK_TEST_HANG_MARKER"
    if [ -n "${HOOK_TEST_RELEASE_FILE:-}" ]; then
        # Deterministic hold: keep linting until the test releases us, with a
        # bound so a broken test cannot hang the suite.
        for _ in $(seq 1 600); do
            [ -e "$HOOK_TEST_RELEASE_FILE" ] && break
            sleep 0.1
        done
    else
        sleep "${HOOK_TEST_HANG_SECONDS:-30}"
    fi
fi
exit "${HOOK_TEST_EXIT:-0}"
CARGO
    chmod +x "$scratch/bin/cargo"
}

# Give the developer something dirty in every state Git can represent.
# $1 = repository directory
seed_dirty_working_tree() {
    local repo=$1
    printf 'fn untracked() {}\n' > "$repo/untracked.rs"
    printf 'unstaged edit\n' >> "$repo/tracked.rs"
    git -C "$repo" add -N untracked.rs
    printf 'intent to add\n' >> "$repo/untracked.rs"
    printf 'staged but edited again\n' >> "$repo/staged.rs"
    git -C "$repo" add staged.rs
    printf 'unstaged edit after staging\n' >> "$repo/staged.rs"
    printf 'deleted in index\n' > "$repo/removed.rs"
    git -C "$repo" add removed.rs
    git -C "$repo" commit -qm 'add files to delete'
    git -C "$repo" rm -q removed.rs
    printf 'resurrected in the working tree\n' > "$repo/removed.rs"
}

# Capture everything a developer could lose: the exact bytes of every file,
# plus the staged tree and the full working-tree diff against the index.
snapshot_worktree() {
    local repo=$1 out=$2 path
    (
        cd "$repo" || exit 1
        while IFS= read -r -d '' path; do
            printf '%s %s\n' "$(git hash-object -- "$path")" "$path"
        done < <(find . -path ./.git -prune -o -type f -print0 | sort -z)
        echo '--- index ---'
        git write-tree
        echo '--- diff ---'
        git diff
    ) > "$out"
}

new_repo() {
    local repo=$1
    mkdir -p "$repo"
    git -C "$repo" init -q
    git -C "$repo" config user.email fixture@example.com
    git -C "$repo" config user.name Fixture
    # A committed manifest, so the hook has the commit's own workspace to lint.
    printf '[package]\nname = "fixture"\nversion = "0.1.0"\nedition = "2021"\n' > "$repo/Cargo.toml"
    printf 'fn tracked() {}\n' > "$repo/tracked.rs"
    printf 'fn staged() {}\n' > "$repo/staged.rs"
    git -C "$repo" add Cargo.toml tracked.rs staged.rs
    git -C "$repo" commit -qm 'initial'
}

setup_bin
export PATH="$scratch/bin:$PATH"

# --- Original scenarios: verdicts -----------------------------------------
repo="$scratch/verdicts"
new_repo "$repo"
(
    cd "$repo"
    HOOK_TEST_OUTPUT='warning: the following packages contain code that will be rejected by a future version of Rust: dependency v1.0.0' bash "$hook" >/dev/null
) || fail 'hook rejected a dependency future-incompatibility notice'

if (
    cd "$repo"
    HOOK_TEST_OUTPUT='warning: unused variable: `value`' bash "$hook" >/dev/null
); then
    fail 'compiler warning accepted'
else
    pass 'compiler warning blocks the commit'
fi

if (
    cd "$repo"
    HOOK_TEST_OUTPUT='warning: this expression creates a reference which is immediately dereferenced' bash "$hook" >/dev/null
); then
    fail 'Clippy warning accepted'
else
    pass 'Clippy warning blocks the commit'
fi

if (
    cd "$repo"
    HOOK_TEST_OUTPUT='error: unused import: `std::fmt`' HOOK_TEST_EXIT=101 bash "$hook" >/dev/null
); then
    fail 'denied compiler error accepted'
else
    pass 'denied compiler error blocks the commit'
fi

if (
    cd "$repo"
    HOOK_TEST_EXIT=101 HOOK_TEST_OUTPUT='error: compilation failed' bash "$hook" >/dev/null
); then
    fail 'Cargo failure accepted'
else
    pass 'Cargo failure blocks the commit'
fi

repo="$scratch/suppression"
new_repo "$repo"
printf '#[allow(dead_code)]\nfn tracked() {}\n' > "$repo/tracked.rs"
git -C "$repo" add tracked.rs
if (cd "$repo" && bash "$hook" >/dev/null 2>&1); then
    fail 'lint suppression accepted'
else
    pass 'staged lint suppression blocks the commit'
fi

# --- Acceptance line 3: a clean commit with a dirty working tree passes ----
repo="$scratch/clean-commit-dirty-tree"
new_repo "$repo"
seed_dirty_working_tree "$repo"
printf 'fn staged_clean() {}\n' > "$repo/clean.rs"
git -C "$repo" add clean.rs
snapshot_worktree "$repo" "$scratch/clean-commit-dirty-tree.before"
if (cd "$repo" && bash "$hook" >/dev/null 2>&1); then
    pass 'clean commit with a dirty working tree passes'
else
    fail 'clean commit with a dirty working tree was blocked'
fi
snapshot_worktree "$repo" "$scratch/clean-commit-dirty-tree.after"
if diff -u "$scratch/clean-commit-dirty-tree.before" "$scratch/clean-commit-dirty-tree.after"; then
    pass 'passing hook left every unstaged and untracked change untouched'
else
    fail 'passing hook altered the working tree'
fi

# --- Acceptance line 3: a staged warning fails ----------------------------
repo="$scratch/staged-warning"
new_repo "$repo"
seed_dirty_working_tree "$repo"
printf 'fn staged_warning() { let CHA27_STAGED_WARNING = 1; }\n' > "$repo/warning.rs"
git -C "$repo" add warning.rs
snapshot_worktree "$repo" "$scratch/staged-warning.before"
if (
    cd "$repo"
    # The warning comes from the staged content the hook lints, not from a
    # canned output, so this covers "a staged warning fails" as reported.
    HOOK_TEST_WARN_ON_MARKER='CHA27_STAGED_WARNING' bash "$hook" > "$scratch/staged-warning.log" 2>&1
); then
    fail 'staged warning was accepted'
elif ! grep -q 'Clippy warnings found' "$scratch/staged-warning.log"; then
    fail 'staged warning was rejected for the wrong reason'
    cat "$scratch/staged-warning.log" >&2
else
    pass 'staged warning fails the commit'
fi
snapshot_worktree "$repo" "$scratch/staged-warning.after"
if diff -u "$scratch/staged-warning.before" "$scratch/staged-warning.after"; then
    pass 'failing hook left every unstaged and untracked change untouched'
else
    fail 'failing hook altered the working tree'
fi

# --- Interrupt: the working tree survives, and the hook stops promptly ----
# The exit code alone is not proof: bash reports 130 for an untrapped SIGINT
# while `wait` is blocked and then carries on. These checks require the hook
# to stop well before cargo would have finished, to kill cargo, and to release
# the lock, so removing the trap (or ignoring the signal) fails the test.
repo="$scratch/interrupt"
new_repo "$repo"
seed_dirty_working_tree "$repo"
printf 'fn staged_clean() {}\n' > "$repo/clean.rs"
git -C "$repo" add clean.rs
snapshot_worktree "$repo" "$scratch/interrupt.before"
marker="$scratch/interrupt.marker"
cargo_pid_file="$scratch/interrupt.cargo.pid"
interrupt_log="$scratch/interrupt.log"
interrupt_status="$scratch/interrupt.status"
interrupt_elapsed="$scratch/interrupt.elapsed"
# A terminal delivers Ctrl-C to the hook and cargo together, so the test puts
# the hook in its own process group (job control) and signals that group.
(
    set -m
    cd "$repo" || exit 1
    HOOK_TEST_HANG_MARKER="$marker" HOOK_TEST_HANG_SECONDS=60 HOOK_TEST_PID_FILE="$cargo_pid_file" \
        bash "$hook" > "$interrupt_log" 2>&1 &
    hook_pid=$!
    for _ in $(seq 1 100); do
        [ -e "$marker" ] && break
        sleep 0.1
    done
    if [ ! -e "$marker" ]; then
        echo 'ERROR: hook never reached cargo' >&2
        kill -KILL -- -"$hook_pid" 2>/dev/null || true
        exit 1
    fi
    started=$(date +%s)
    kill -INT -- -"$hook_pid" 2>/dev/null || true
    wait "$hook_pid"
    printf '%s\n' "$?" > "$interrupt_status"
    printf '%s\n' "$(( $(date +%s) - started ))" > "$interrupt_elapsed"
    # Nothing should survive the interrupt; make sure of it before moving on.
    kill -KILL -- -"$hook_pid" 2>/dev/null || true
) || true
if [ -e "$marker" ] && grep -qx '130' "$interrupt_status" 2>/dev/null; then
    pass 'hook was interrupted while cargo was linting'
else
    fail "hook was not interrupted while linting (status: $(cat "$interrupt_status" 2>/dev/null || echo none))"
fi
if [ -e "$interrupt_elapsed" ] && [ "$(cat "$interrupt_elapsed")" -lt 20 ]; then
    pass 'the interrupt stopped the hook promptly instead of after cargo'
else
    fail "the hook did not stop promptly on interrupt ($(cat "$interrupt_elapsed" 2>/dev/null || echo '?')s)"
fi
snapshot_worktree "$repo" "$scratch/interrupt.after"
if diff -u "$scratch/interrupt.before" "$scratch/interrupt.after"; then
    pass 'interrupted hook left every unstaged and untracked change untouched'
else
    fail 'interrupted hook altered the working tree'
fi
if [ -e "$repo/.git/chartml-pre-commit/lock" ]; then
    fail 'interrupted hook left its lint lock behind'
else
    pass 'interrupted hook released its lint lock'
fi
if kill -0 "$(cat "$cargo_pid_file")" 2>/dev/null; then
    fail 'interrupted hook left cargo running'
    kill -KILL "$(cat "$cargo_pid_file")" 2>/dev/null || true
else
    pass 'interrupted hook stopped its cargo child'
fi

# --- A hook signalled on its own still stops cargo ------------------------
# A terminal signals the whole group, but killing just the hook (for example
# closing the editor that spawned it) must not leave cargo racing the next
# hook on the same target directory.
repo="$scratch/hook-only-signal"
new_repo "$repo"
printf 'fn staged_clean() {}\n' > "$repo/clean.rs"
git -C "$repo" add clean.rs
marker="$scratch/hook-only-signal.marker"
cargo_pid_file="$scratch/hook-only-signal.cargo.pid"
(
    cd "$repo" || exit 1
    HOOK_TEST_HANG_MARKER="$marker" HOOK_TEST_HANG_SECONDS=30 HOOK_TEST_PID_FILE="$cargo_pid_file" \
        bash "$hook" >/dev/null 2>&1 &
    hook_pid=$!
    for _ in $(seq 1 100); do
        [ -e "$marker" ] && break
        sleep 0.1
    done
    if [ ! -e "$marker" ]; then
        echo 'ERROR: hook never reached cargo' >&2
        kill -KILL "$hook_pid" 2>/dev/null || true
        exit 1
    fi
    kill -TERM "$hook_pid" 2>/dev/null || true
    wait "$hook_pid" 2>/dev/null || true
    # Cleanup runs before the hook exits; give the orphan check a moment.
    for _ in $(seq 1 50); do
        kill -0 "$(cat "$cargo_pid_file")" 2>/dev/null || break
        sleep 0.1
    done
    if kill -0 "$(cat "$cargo_pid_file")" 2>/dev/null; then
        echo 'ORPHANED' > "$scratch/hook-only-signal.result"
        kill -KILL "$(cat "$cargo_pid_file")" 2>/dev/null || true
    fi
) || true
if [ -e "$scratch/hook-only-signal.result" ]; then
    fail 'hook signalled on its own left cargo running'
else
    pass 'hook signalled on its own stopped cargo'
fi
if [ -e "$repo/.git/chartml-pre-commit/lock" ]; then
    fail 'hook signalled on its own left its lint lock behind'
else
    pass 'hook signalled on its own released its lint lock'
fi

# --- Snapshot exactness: the hook lints the commit, not the working tree --
repo="$scratch/snapshot-content"
new_repo "$repo"
pwd_log="$scratch/snapshot-content.pwd"
(
    cd "$repo"
    HOOK_TEST_REPO_ROOT=$(cd -- "$repo" && pwd -P) HOOK_TEST_PWD_LOG="$pwd_log" bash "$hook" > "$scratch/snapshot-content.log" 2>&1
) || fail 'hook failed while recording the linted directory'
linted=$(cat "$pwd_log")
linted_resolved=$(cd -- "$linted" 2>/dev/null && pwd -P || printf '%s' "$linted")
case "$linted/" in
    "$repo"/*) fail "hook linted inside the working tree: $linted" ;;
    *) pass "hook linted a copy outside the working tree" ;;
esac
if grep -q 'lint copy is inside the working tree' "$scratch/snapshot-content.log"; then
    fail 'the lint copy sits inside the working tree, so Cargo would read unstaged files'
else
    pass 'the lint copy cannot inherit the working tree manifest or cargo config'
fi
if grep -qF "lint copy:  $linted" "$scratch/snapshot-content.log" ||
    grep -qF "lint copy:  $linted_resolved" "$scratch/snapshot-content.log"; then
    reported=1
else
    reported=0
fi
if [ "$reported" -eq 1 ] && [ "$(grep -c 'lint copy:' "$scratch/snapshot-content.log")" -eq 1 ]; then
    pass 'the hook reports the lint copy exactly once'
else
    fail "the hook reported the lint copy $(grep -c 'lint copy:' "$scratch/snapshot-content.log") times"
fi
if [ "$reported" -eq 1 ] &&
    grep -qE 'target dir: .*/\.git/chartml-pre-commit/target$' "$scratch/snapshot-content.log" &&
    grep -qE 'remove them: rm -rf -- .*/chartml-pre-commit\.[A-Za-z0-9]+ .*/chartml-pre-commit/target$' "$scratch/snapshot-content.log"; then
    pass 'the hook says where the lint copy and target dir live and how to remove them'
else
    fail 'the hook does not report the lint copy and target dir locations'
    cat "$scratch/snapshot-content.log" >&2
fi

# A file deleted from the index must be gone from the lint copy, and staged
# content must be what the hook sees even when the working copy differs.
repo="$scratch/snapshot-exact"
new_repo "$repo"
printf 'fn stale() {}\n' > "$repo/stale.rs"
git -C "$repo" add stale.rs
git -C "$repo" commit -qm 'add stale file'
git -C "$repo" rm -q stale.rs
printf 'fn staged_body() {}\n' > "$repo/tracked.rs"
git -C "$repo" add tracked.rs
printf 'fn working_tree_body() {}\n' > "$repo/tracked.rs"
(
    cd "$repo"
    HOOK_TEST_PWD_LOG="$scratch/snapshot-exact.pwd" bash "$hook" >/dev/null 2>&1
) || fail 'hook failed on a repo with staged deletions'
snapshot=$(snapshot_dir "$repo")
if [ -e "$snapshot/stale.rs" ]; then
    fail 'file deleted from the index survived in the lint copy'
else
    pass 'file deleted from the index is absent from the lint copy'
fi
if grep -q 'staged_body' "$snapshot/tracked.rs" && ! grep -q 'working_tree_body' "$snapshot/tracked.rs"; then
    pass 'snapshot holds staged content, not the working copy'
else
    fail 'snapshot holds working-tree content instead of staged content'
fi

# --- The verdict depends on the commit, not the working tree --------------
# The ticket's core failure: unstaged edits blocked a clean commit, and the
# hook could pass a commit that would not pass on its own. The fake cargo
# warns only when the content it was given contains a marker, so these two
# cases prove the verdict follows the commit rather than the working tree.
repo="$scratch/verdict-follows-index"
new_repo "$repo"
printf 'fn tracked() { let CHA27_BASELINE = 1; }\n' > "$repo/tracked.rs"
git -C "$repo" add tracked.rs
git -C "$repo" commit -qm 'tracked with baseline marker'
printf 'fn working_tree_only() { let CHA27_UNSTAGED_WARNING = 1; }\n' > "$repo/tracked.rs"
(
    cd "$repo"
    HOOK_TEST_WARN_ON_MARKER='CHA27_UNSTAGED_WARNING' bash "$hook" >/dev/null 2>&1
) || {
    fail 'an unstaged warning blocked a clean commit'
    unstaged_blocked=1
}
if [ "${unstaged_blocked:-0}" -eq 0 ]; then
    pass 'warning in the working tree only does not block the commit'
fi

printf 'fn staged_warns() { let CHA27_STAGED_WARNING = 1; }\n' > "$repo/tracked.rs"
git -C "$repo" add tracked.rs
printf 'fn working_tree_clean() {}\n' > "$repo/tracked.rs"
if (
    cd "$repo"
    HOOK_TEST_WARN_ON_MARKER='CHA27_STAGED_WARNING' bash "$hook" >/dev/null 2>&1
); then
    fail 'staged warning passed because the working tree looked clean'
else
    pass 'staged warning blocks the commit even with a clean working tree'
fi

# --- A snapshot that differs from the commit is refused -------------------
# A smudge filter rewrites content at checkout, so the materialised snapshot
# cannot match the committed tree. The hook must refuse rather than lint
# something the commit would not contain.
repo="$scratch/snapshot-mismatch"
new_repo "$repo"
printf '*.rs filter=cha27smudge\n' > "$repo/.gitattributes"
git -C "$repo" add .gitattributes
git -C "$repo" commit -qm 'attribute the filter'
git -C "$repo" config filter.cha27smudge.smudge "sed 's/fn tracked/fn smudged/'"
git -C "$repo" config filter.cha27smudge.clean cat
printf 'fn tracked() {}\n' > "$repo/tracked.rs"
git -C "$repo" add tracked.rs
if (cd "$repo" && bash "$hook" > "$scratch/snapshot-mismatch.log" 2>&1); then
    fail 'hook linted a snapshot that differs from the commit'
elif grep -q 'does not match the committed content' "$scratch/snapshot-mismatch.log"; then
    pass 'snapshot that differs from the commit is refused'
else
    fail 'snapshot mismatch was rejected for the wrong reason'
    cat "$scratch/snapshot-mismatch.log" >&2
fi

# --- Unstaged files cannot change the verdict -----------------------------
# Cargo searches upwards for a manifest and for .cargo/config.toml. If the lint
# copy sits inside the working tree it finds the developer's unstaged ones, so
# these two cases are what "lints exactly the commit" means in practice.
repo="$scratch/isolation"
new_repo "$repo"
mkdir -p "$repo/.cargo"
printf '[build]\nrustflags = ["-A", "warnings"]\n' > "$repo/.cargo/config.toml"
printf 'fn tracked() { let CHA27_COMMITTED_WARNING = 1; }\n' > "$repo/tracked.rs"
git -C "$repo" add tracked.rs
if (
    cd "$repo"
    HOOK_TEST_WARN_ON_MARKER='CHA27_COMMITTED_WARNING' bash "$hook" > "$scratch/isolation.log" 2>&1
); then
    fail 'an untracked .cargo/config.toml suppressed a warning that is in the commit'
else
    pass 'an untracked .cargo/config.toml cannot suppress a committed warning'
fi

# A commit that stages no manifest must still be linted as the commit's own
# content, not by walking up to the working tree's manifest.
repo="$scratch/isolation-manifest"
new_repo "$repo"
printf 'fn tracked() { let CHA27_MANIFEST_WARNING = 1; }\n' > "$repo/tracked.rs"
git -C "$repo" add tracked.rs
git -C "$repo" rm -q --cached Cargo.toml 2>/dev/null || true
if (
    cd "$repo"
    HOOK_TEST_WARN_ON_MARKER='CHA27_MANIFEST_WARNING' bash "$hook" > "$scratch/isolation-manifest.log" 2>&1
); then
    fail 'a commit without a manifest was linted through the working tree'
elif ! grep -q 'no Cargo.toml' "$scratch/isolation-manifest.log"; then
    fail 'a commit without a manifest was rejected for the wrong reason'
    cat "$scratch/isolation-manifest.log" >&2
else
    pass 'a commit without a manifest is refused instead of linting the working tree'
fi

# --- Settings outside the commit cannot change the verdict ---------------
# Cargo merges .cargo/config.toml from the copy's ancestors and from
# CARGO_HOME, and reads RUSTFLAGS from the environment. None of that is part of
# the commit, so a rustflags that silences warnings must not make a commit with
# a real warning pass.
repo="$scratch/isolation-env"
new_repo "$repo"
# The ancestor config must be outside the repository (a TMPDIR inside it is
# refused for a different reason), yet still an ancestor of the lint copy.
outer="$scratch/isolation-outer"
mkdir -p "$outer/.cargo"
printf '[build]\nrustflags = ["-A", "warnings"]\n' > "$outer/.cargo/config.toml"
printf 'fn tracked() { let CHA27_ANCESTOR_WARNING = 1; }\n' > "$repo/tracked.rs"
git -C "$repo" add tracked.rs
if (
    cd "$repo"
    TMPDIR="$outer" HOOK_TEST_WARN_ON_MARKER='CHA27_ANCESTOR_WARNING' bash "$hook" > "$scratch/isolation-env.log" 2>&1
); then
    fail 'a cargo config in an ancestor directory suppressed a committed warning'
elif ! grep -q 'CHA27_ANCESTOR_WARNING\|Clippy warnings found' "$scratch/isolation-env.log"; then
    fail 'the ancestor-config case failed for the wrong reason'
    cat "$scratch/isolation-env.log" >&2
else
    pass 'a cargo config in an ancestor directory cannot suppress a committed warning'
fi
if (
    cd "$repo"
    RUSTFLAGS='-A warnings' HOOK_TEST_WARN_ON_MARKER='CHA27_ANCESTOR_WARNING' bash "$hook" > "$scratch/isolation-rustflags.log" 2>&1
); then
    fail 'RUSTFLAGS from the environment suppressed a committed warning'
else
    pass 'RUSTFLAGS from the environment cannot suppress a committed warning'
fi
# RUSTC_WRAPPER can rewrite the compiler invocation itself, so it is cleared
# for the lint run too.
mkdir -p "$scratch/wrapper"
printf '#!/usr/bin/env bash\nexec "$@" -A warnings\n' > "$scratch/wrapper/rustc-wrapper"
chmod +x "$scratch/wrapper/rustc-wrapper"
printf 'fn tracked() { let CHA27_WRAPPER_WARNING = 1; }\n' > "$repo/tracked.rs"
git -C "$repo" add tracked.rs
if (
    cd "$repo"
    RUSTC_WRAPPER="$scratch/wrapper/rustc-wrapper" HOOK_TEST_WARN_ON_MARKER='CHA27_WRAPPER_WARNING' bash "$hook" > "$scratch/isolation-wrapper.log" 2>&1
); then
    fail 'RUSTC_WRAPPER from the environment suppressed a committed warning'
else
    pass 'RUSTC_WRAPPER from the environment cannot suppress a committed warning'
fi

# --- The hook never rewrites an unstaged tracked file ---------------------
# Cargo rewrites Cargo.lock when it resolves from a directory it should not
# have read. An unstaged lockfile edit must survive a passing and a failing run.
repo="$scratch/untouched-lock"
new_repo "$repo"
# Mirror the real workspace shape: the excluded packages are members, and other
# members remain, so `--workspace --exclude` still has something to lint.
mkdir -p "$repo/crates/chartml-wasm/src" "$repo/crates/core/src"
printf '[workspace]\nresolver = "2"\nmembers = ["crates/core", "crates/chartml-wasm"]\n' > "$repo/Cargo.toml"
printf '[package]\nname = "core"\nversion = "0.1.0"\nedition = "2021"\n' > "$repo/crates/core/Cargo.toml"
printf 'pub fn f() {}\n' > "$repo/crates/core/src/lib.rs"
printf '[package]\nname = "chartml-wasm"\nversion = "0.1.0"\nedition = "2021"\n' > "$repo/crates/chartml-wasm/Cargo.toml"
printf 'pub fn f() {}\n' > "$repo/crates/chartml-wasm/src/lib.rs"
printf '# unstaged developer edit\n' > "$repo/Cargo.lock"
git -C "$repo" add -A
git -C "$repo" commit -qm 'workspace, manifest and lock'
printf '# unstaged developer edit, second pass\n' > "$repo/Cargo.lock"
before=$(git -C "$repo" hash-object Cargo.lock)
(cd "$repo" && bash "$hook" >/dev/null 2>&1) || true
after=$(git -C "$repo" hash-object Cargo.lock)
if [ "$before" = "$after" ]; then
    pass 'a failing hook leaves an unstaged Cargo.lock byte-identical'
else
    fail 'a failing hook rewrote an unstaged Cargo.lock'
fi
printf 'fn staged_clean() {}\n' > "$repo/clean.rs"
git -C "$repo" add clean.rs
before=$(git -C "$repo" hash-object Cargo.lock)
(cd "$repo" && bash "$hook" >/dev/null 2>&1) || true
after=$(git -C "$repo" hash-object Cargo.lock)
if [ "$before" = "$after" ]; then
    pass 'a passing hook leaves an unstaged Cargo.lock byte-identical'
else
    fail 'a passing hook rewrote an unstaged Cargo.lock'
fi

# --- Both hooks starting at once must both succeed -----------------------
# The lock is taken before the index is read, so two hooks cannot collide in
# `git write-tree` (which needs the real index lock) and be misreported as
# unresolved conflicts.
repo="$scratch/simultaneous"
new_repo "$repo"
printf 'fn staged_clean() {}\n' > "$repo/clean.rs"
git -C "$repo" add clean.rs
simultaneous_failures=0
for _ in 1 2 3; do
    if ! (
        cd "$repo" || exit 1
        bash "$hook" > "$scratch/simultaneous.1.log" 2>&1 &
        first=$!
        bash "$hook" > "$scratch/simultaneous.2.log" 2>&1 &
        second=$!
        status=0
        wait "$first" || status=1
        wait "$second" || status=1
        exit "$status"
    ); then
        simultaneous_failures=$((simultaneous_failures + 1))
        grep -E '❌' "$scratch/simultaneous.1.log" "$scratch/simultaneous.2.log" >&2 || true
    fi
done
if [ "$simultaneous_failures" -eq 0 ]; then
    pass 'two simultaneous hooks both accept a clean commit'
else
    fail "a simultaneous hook refused a clean commit ($simultaneous_failures of 3 rounds)"
fi

# --- Linked worktrees keep separate lint copies --------------------------
repo="$scratch/linked"
new_repo "$repo"
git -C "$repo" worktree add -q -b linked-branch "$scratch/linked-wt" >/dev/null
(
    cd "$repo"
    HOOK_TEST_PWD_LOG="$scratch/linked-main.pwd" bash "$hook" >/dev/null 2>&1
) || fail 'hook failed in the main worktree'
(
    cd "$scratch/linked-wt"
    HOOK_TEST_PWD_LOG="$scratch/linked-wt.pwd" bash "$hook" >/dev/null 2>&1
) || fail 'hook failed in a linked worktree'
main_linted=$(cat "$scratch/linked-main.pwd")
wt_linted=$(cat "$scratch/linked-wt.pwd")
if [ "$main_linted" = "$wt_linted" ]; then
    fail "linked worktree reused the main worktree's snapshot: $wt_linted"
else
    pass "linked worktree has its own snapshot: $wt_linted"
fi

# --- Concurrent hooks in one worktree are serialised, not corrupted --------
repo="$scratch/concurrent"
new_repo "$repo"
seed_dirty_working_tree "$repo"
printf 'fn staged_clean() {}\n' > "$repo/clean.rs"
git -C "$repo" add clean.rs
snapshot_worktree "$repo" "$scratch/concurrent.before"
first_marker="$scratch/concurrent.marker"
first_log="$scratch/concurrent.first.log"
second_log="$scratch/concurrent.second.log"
(
    cd "$repo"
    HOOK_TEST_HANG_MARKER="$first_marker" HOOK_TEST_HANG_SECONDS=3 bash "$hook" > "$first_log" 2>&1 &
    first_pid=$!
    for _ in $(seq 1 100); do
        [ -e "$first_marker" ] && break
        sleep 0.1
    done
    [ -e "$first_marker" ] || { echo 'ERROR: first hook never reached cargo' >&2; exit 1; }
    bash "$hook" > "$second_log" 2>&1 || { echo 'ERROR: concurrent hook failed' >&2; exit 1; }
    wait "$first_pid" || { echo 'ERROR: first hook failed' >&2; exit 1; }
) || fail 'concurrent hooks did not both succeed'
if grep -q 'Waiting for another pre-commit hook' "$second_log"; then
    pass 'second hook waited for the first instead of corrupting the snapshot'
else
    fail 'second hook did not report waiting for the lock'
fi
snapshot_worktree "$repo" "$scratch/concurrent.after"
if diff -u "$scratch/concurrent.before" "$scratch/concurrent.after"; then
    pass 'concurrent hooks left every unstaged and untracked change untouched'
else
    fail 'concurrent hooks altered the working tree'
fi

# --- Stale lock left by a dead hook does not block commits ----------------
repo="$scratch/stale-lock"
new_repo "$repo"
mkdir -p "$repo/.git/chartml-pre-commit"
printf '999999\n' > "$repo/.git/chartml-pre-commit/lock"
if (cd "$repo" && bash "$hook" >/dev/null 2>&1); then
    pass 'stale lock from a dead hook is reclaimed'
else
    fail 'stale lock from a dead hook blocked the commit'
fi

# --- Real git commits honour the hook -------------------------------------
# The tests above call the hook directly; these go through `git commit`, the
# path contributors actually use, including a partial commit that Git runs
# against a temporary index.
hooks="$scratch/real-commit-hooks"
mkdir -p "$hooks"
printf '#!/usr/bin/env bash\nexec bash %s "$@"\n' "$hook" > "$hooks/pre-commit"
chmod +x "$hooks/pre-commit"
repo="$scratch/real-commit"
new_repo "$repo"
git -C "$repo" config core.hooksPath "$hooks"
printf 'fn tracked() { let CHA27_COMMITTED = 1; }\n' > "$repo/tracked.rs"
git -C "$repo" add tracked.rs
printf 'fn tracked() { let CHA27_UNSTAGED = 1; }\n' > "$repo/tracked.rs"
if (
    cd "$repo"
    # The real cargo never sees the unstaged marker, so only the snapshot's
    # content decides this outcome.
    git commit -qm 'clean commit from a dirty tree' > "$scratch/real-commit.log" 2>&1
); then
    pass 'a real git commit passes with only unstaged changes warning'
else
    fail 'a real git commit was blocked by unstaged content'
    cat "$scratch/real-commit.log" >&2
fi
if [ "$(git -C "$repo" diff --name-only | wc -l | tr -d ' ')" -eq 1 ] &&
    grep -q 'CHA27_UNSTAGED' "$repo/tracked.rs"; then
    pass 'a real git commit left the unstaged edit in place'
else
    fail 'a real git commit altered the unstaged edit'
fi

printf 'fn warn_now() { let CHA27_STAGED_WARNING = 1; }\n' > "$repo/staged_warning.rs"
git -C "$repo" add staged_warning.rs
printf 'fn warn_now() {}\n' > "$repo/staged_warning.rs"
if (
    cd "$repo"
    HOOK_TEST_WARN_ON_MARKER='CHA27_STAGED_WARNING' git commit -qm 'warn' > "$scratch/real-commit-warn.log" 2>&1
); then
    fail 'a real git commit accepted staged warning content'
else
    pass 'a real git commit is blocked by staged warning content'
fi

# A partial commit only commits the named paths; the hook must lint that
# commit, not the whole index.
repo="$scratch/real-partial-commit"
new_repo "$repo"
git -C "$repo" config core.hooksPath "$hooks"
printf 'fn tracked() { let CHA27_OTHER_PATH = 1; }\n' > "$repo/tracked.rs"
git -C "$repo" add tracked.rs
printf 'fn staged() { let cha27_partial = 1; }\n' > "$repo/staged.rs"
git -C "$repo" add staged.rs
printf 'fn staged() { let cha27_partial = 2; }\n' > "$repo/staged.rs"
if (
    cd "$repo"
    HOOK_TEST_WARN_ON_MARKER='CHA27_OTHER_PATH' git commit -qm 'partial' -- staged.rs > "$scratch/real-partial.log" 2>&1
); then
    pass 'a partial commit lints only the content it commits'
else
    fail 'a partial commit was blocked by content it does not commit'
    cat "$scratch/real-partial.log" >&2
fi

# --- Refusal does not depend on the executable bit ------------------------
# clippy reads content, so a checkout where the filesystem cannot represent
# the executable bit (core.filemode=false, e.g. Git for Windows) must still
# commit. Comparing whole tree ids used to refuse every commit here.
repo="$scratch/filemode"
new_repo "$repo"
printf '#!/bin/sh\necho tool\n' > "$repo/tool.sh"
chmod +x "$repo/tool.sh"
git -C "$repo" add tool.sh
git -C "$repo" commit -qm 'add executable'
git -C "$repo" config core.filemode false
printf 'fn tracked() { let cha27 = 1; }\n' > "$repo/tracked.rs"
git -C "$repo" add tracked.rs
if (cd "$repo" && bash "$hook" > "$scratch/filemode.log" 2>&1); then
    pass 'a checkout without executable bits still commits'
else
    fail 'core.filemode=false refused a clean commit'
    cat "$scratch/filemode.log" >&2
fi

# --- A second hook that gives up must not disturb the running one ---------
# A hook that exits before taking the lock used to delete the active hook's
# clippy log, so a clean commit failed for an unrelated reason. The conflict
# is built in a separate clone so the repository the hook is linting is never
# disturbed, and the test asserts the conflict really is in the index.
repo="$scratch/second-hook-cleanup"
new_repo "$repo"
printf 'base\n' > "$repo/held.rs"
git -C "$repo" add held.rs
git -C "$repo" commit -qm 'add the file both branches change'
printf 'fn staged_clean() {}\n' > "$repo/clean.rs"
git -C "$repo" add clean.rs
conflict_repo="$scratch/second-hook-conflict"
git clone -q "$repo" "$conflict_repo"
(
    cd "$conflict_repo" || exit 1
    git config user.email fixture@example.com
    git config user.name Fixture
    git checkout -q -b conflicting-side
    printf 'theirs\n' > held.rs
    git commit -qam theirs
    git checkout -q -
    printf 'ours\n' > held.rs
    git commit -qam ours
    git merge conflicting-side >/dev/null 2>&1 || true
) || fail 'could not build the conflicting index for the second hook'
unmerged=$(git -C "$conflict_repo" ls-files -u | wc -l | tr -d ' ')
if [ "$unmerged" -gt 0 ]; then
    pass 'the second hook has a genuinely conflicting index to exit on'
else
    fail 'the second hook fixture has no conflict, so it would not exit early'
fi
first_marker="$scratch/second-hook-cleanup.marker"
first_release="$scratch/second-hook-cleanup.release"
first_log="$scratch/second-hook-cleanup.first.log"
(
    cd "$repo" || exit 1
    HOOK_TEST_HANG_MARKER="$first_marker" HOOK_TEST_RELEASE_FILE="$first_release" bash "$hook" > "$first_log" 2>&1 &
    first_pid=$!
    for _ in $(seq 1 100); do
        [ -e "$first_marker" ] && break
        sleep 0.1
    done
    if [ ! -e "$first_marker" ]; then
        echo 'ERROR: first hook never reached cargo' >&2
        kill -KILL "$first_pid" 2>/dev/null || true
        exit 1
    fi
    # This hook exits at the unresolved-index check, before taking the lock.
    (
        cd "$conflict_repo" || exit 1
        bash "$hook" > "$scratch/second-hook-cleanup.conflict.log" 2>&1
    ) || true
    # Only now let the first hook finish linting.
    : > "$first_release"
    if wait "$first_pid"; then
        :
    else
        echo 'FIRST_FAILED' > "$scratch/second-hook-cleanup.result"
    fi
) || true
if grep -q 'unresolved conflicts' "$scratch/second-hook-cleanup.conflict.log" 2>/dev/null; then
    pass 'the second hook exited at the unresolved-index check before taking the lock'
else
    fail 'the second hook did not exit at the unresolved-index check'
    cat "$scratch/second-hook-cleanup.conflict.log" >&2 2>/dev/null || true
fi
if [ -e "$scratch/second-hook-cleanup.result" ]; then
    fail 'a second hook that exited early broke the running hook'
    tail -5 "$first_log" >&2
elif grep -q '✅ Clippy clean' "$first_log"; then
    pass 'a second hook that exits early leaves the running hook clean'
else
    fail 'the running hook did not finish cleanly'
    tail -5 "$first_log" >&2
fi

# --- A hook killed with SIGKILL leaves no permanent litter -----------------
# Traps cannot run on SIGKILL, so the next hook must reclaim the dead run's
# per-pid lock and log files, and must not report waiting on a dead holder.
repo="$scratch/killed-hook"
new_repo "$repo"
printf 'fn staged_clean() {}\n' > "$repo/clean.rs"
git -C "$repo" add clean.rs
marker="$scratch/killed-hook.marker"
(
    cd "$repo" || exit 1
    HOOK_TEST_HANG_MARKER="$marker" HOOK_TEST_HANG_SECONDS=30 bash "$hook" >/dev/null 2>&1 &
    hook_pid=$!
    for _ in $(seq 1 100); do
        [ -e "$marker" ] && break
        sleep 0.1
    done
    if [ ! -e "$marker" ]; then
        echo 'ERROR: hook never reached cargo' >&2
        kill -KILL "$hook_pid" 2>/dev/null || true
        exit 1
    fi
    kill -KILL "$hook_pid" 2>/dev/null || true
    wait "$hook_pid" 2>/dev/null || true
) || true
state_dir="$repo/.git/chartml-pre-commit"
if [ -e "$state_dir/lock" ]; then
    if (cd "$repo" && bash "$hook" > "$scratch/killed-hook.log" 2>&1); then
        pass 'a commit after a SIGKILLed hook succeeds'
    else
        fail 'a commit after a SIGKILLed hook was blocked'
        cat "$scratch/killed-hook.log" >&2
    fi
    if ls "$state_dir"/clippy.*.log >/dev/null 2>&1 || ls -d "$state_dir"/lock.stale.* >/dev/null 2>&1; then
        fail 'per-run files from a SIGKILLed hook were never reclaimed'
        ls -1 "$state_dir" >&2
    else
        pass 'per-run files from a SIGKILLed hook are reclaimed'
    fi
    if grep -q 'Waiting for another pre-commit hook' "$scratch/killed-hook.log"; then
        fail 'the next hook reported waiting on a dead holder'
    else
        pass 'the next hook does not wait on a dead holder'
    fi
else
    fail 'the killed hook left no lock to reclaim, so the test proved nothing'
fi

# --- The fixtures mirror the real workspace shape ------------------------
# `--workspace --exclude chartml-wasm --exclude chartml-wasm-datafusion` needs
# members left over, as the real workspace has. A fixture whose only members
# are the excluded ones would make clippy fail for the wrong reason.
repo="$scratch/workspace-shape"
mkdir -p "$repo"
git -C "$repo" init -q
git -C "$repo" config user.email fixture@example.com
git -C "$repo" config user.name Fixture
mkdir -p "$repo/crates/chartml-wasm/src" "$repo/crates/chartml-wasm-datafusion/src" "$repo/crates/core/src"
printf '[workspace]\nresolver = "2"\nmembers = ["crates/core", "crates/chartml-wasm", "crates/chartml-wasm-datafusion"]\n' > "$repo/Cargo.toml"
for crate in core chartml-wasm chartml-wasm-datafusion; do
    printf '[package]\nname = "%s"\nversion = "0.1.0"\nedition = "2021"\n' "$crate" > "$repo/crates/$crate/Cargo.toml"
    printf 'pub fn f() {}\n' > "$repo/crates/$crate/src/lib.rs"
done
git -C "$repo" add -A
if (cd "$repo" && bash "$hook" > "$scratch/workspace-shape.log" 2>&1); then
    pass 'the hook lints a workspace shaped like this repository'
else
    fail 'the hook failed on a workspace shaped like this repository'
    cat "$scratch/workspace-shape.log" >&2
fi

# --- TMPDIR inside the worktree is refused, never used -------------------
# A lint copy under the worktree would let Cargo read the working tree, which
# is the defect this hook exists to remove, and .tmp//.cache are plausible
# TMPDIR values. The hook must refuse rather than lint from inside the repo.
repo="$scratch/tmpdir-inside"
new_repo "$repo"
mkdir -p "$repo/.tmp"
printf 'fn tracked() { let CHA27_TMPDIR_WARNING = 1; }\n' > "$repo/tracked.rs"
git -C "$repo" add tracked.rs
if (
    cd "$repo"
    TMPDIR="$repo/.tmp" HOOK_TEST_WARN_ON_MARKER='CHA27_TMPDIR_WARNING' bash "$hook" > "$scratch/tmpdir-inside.log" 2>&1
); then
    fail 'a TMPDIR inside the worktree produced a verdict'
elif grep -q 'is inside this repository' "$scratch/tmpdir-inside.log"; then
    pass 'a TMPDIR inside the worktree is refused with the reason'
else
    fail 'a TMPDIR inside the worktree failed for the wrong reason'
    cat "$scratch/tmpdir-inside.log" >&2
fi
if [ -n "$(find "$repo/.tmp" -mindepth 1 -maxdepth 1 -print -quit 2>/dev/null)" ]; then
    fail 'a lint copy was created inside the worktree'
else
    pass 'no lint copy is created inside the worktree'
fi

# --- A half-done reclaim left behind cannot brick the worktree -----------
# A run killed between renaming the lock aside and deleting it leaves
# lock.stale.<pid>. Whatever its form, and even as a directory, it must not
# abort every later commit with no verdict.
repo="$scratch/stale-dir-lock"
new_repo "$repo"
mkdir -p "$repo/.git/chartml-pre-commit/lock.stale.999999"
printf '999999\n' > "$repo/.git/chartml-pre-commit/lock.stale.999999/owner"
printf 'fn staged_clean() {}\n' > "$repo/clean.rs"
git -C "$repo" add clean.rs
if (cd "$repo" && bash "$hook" > "$scratch/stale-dir-lock.log" 2>&1); then
    pass 'a lock.stale.<pid> directory left by a killed reclaim does not block'
else
    fail 'a lock.stale.<pid> directory bricked the worktree'
    cat "$scratch/stale-dir-lock.log" >&2
fi
if [ -e "$repo/.git/chartml-pre-commit/lock.stale.999999" ]; then
    fail 'the leftover lock.stale.<pid> directory was never removed'
else
    pass 'the leftover lock.stale.<pid> directory is reclaimed'
fi

# --- A lock file left by a killed hook is reclaimed ----------------------
repo="$scratch/killed-lock-file"
new_repo "$repo"
mkdir -p "$repo/.git/chartml-pre-commit"
printf '999999\n' > "$repo/.git/chartml-pre-commit/lock"
printf 'fn staged_clean() {}\n' > "$repo/clean.rs"
git -C "$repo" add clean.rs
if (cd "$repo" && bash "$hook" > "$scratch/killed-lock-file.log" 2>&1); then
    pass 'a lock file left by a dead hook is reclaimed'
else
    fail 'a lock file left by a dead hook blocked the commit'
    cat "$scratch/killed-lock-file.log" >&2
fi

# --- A refused run never prints an internal error ------------------------
# An unwritable state directory is reported and stops the commit; the trap then
# runs, and must not fail on an unset variable or emit a script error next to
# the real diagnosis.
repo="$scratch/unwritable-state"
new_repo "$repo"
printf 'fn staged_clean() {}\n' > "$repo/clean.rs"
git -C "$repo" add clean.rs
if [ "$(id -u)" -eq 0 ]; then
    # root ignores directory permissions, so this scenario cannot be created.
    echo 'SKIP: an unwritable state directory cannot be set up as root' >&2
else
    mkdir -p "$repo/.git/chartml-pre-commit"
    chmod 500 "$repo/.git/chartml-pre-commit"
    if (cd "$repo" && bash "$hook" > "$scratch/unwritable-state.log" 2>&1); then
        fail 'an unwritable state directory still produced a verdict'
    else
        pass 'an unwritable state directory is reported'
    fi
    if grep -qE 'unbound variable|command not found|No such file or directory' "$scratch/unwritable-state.log"; then
        fail 'the refusal printed an internal script error'
        cat "$scratch/unwritable-state.log" >&2
    else
        pass 'the refusal prints no internal script error'
    fi
    chmod 700 "$repo/.git/chartml-pre-commit"
fi

# --- The hook works where hard links are unavailable ---------------------
# The lock must not depend on `ln`: exFAT, FAT32 and some network mounts do not
# support hard links, and the repository itself still works there.
repo="$scratch/no-hardlinks"
new_repo "$repo"
mkdir -p "$scratch/no-hardlinks-bin"
printf '#!/usr/bin/env bash\nexit 1\n' > "$scratch/no-hardlinks-bin/ln"
chmod +x "$scratch/no-hardlinks-bin/ln"
printf 'fn staged_clean() {}\n' > "$repo/clean.rs"
git -C "$repo" add clean.rs
if (
    cd "$repo"
    PATH="$scratch/no-hardlinks-bin:$PATH" bash "$hook" > "$scratch/no-hardlinks.log" 2>&1
); then
    pass 'the lock works where hard links are unavailable'
else
    fail 'the lock failed where hard links are unavailable'
    cat "$scratch/no-hardlinks.log" >&2
fi

# --- A rustflags the commit itself carries is honoured -------------------
# The hook must ignore settings from outside the commit, but a .cargo/config.toml
# that is part of the commit is content, so its rustflags must still count.
repo="$scratch/committed-config"
new_repo "$repo"
mkdir -p "$repo/.cargo"
printf '[build]\nrustflags = ["-A", "warnings"]\n' > "$repo/.cargo/config.toml"
printf 'fn tracked() { let CHA27_COMMITTED_SUPPRESSED = 1; }\n' > "$repo/tracked.rs"
git -C "$repo" add .cargo/config.toml tracked.rs
if (
    cd "$repo"
    HOOK_TEST_WARN_ON_MARKER='CHA27_COMMITTED_SUPPRESSED' bash "$hook" > "$scratch/committed-config.log" 2>&1
); then
    pass 'a commit whose own committed rustflags silence a warning passes'
else
    fail 'a commit was blocked by a warning its own committed config silences'
    cat "$scratch/committed-config.log" >&2
fi

# The same config must not silence anything when it is merely sitting in the
# working tree unstaged.
repo="$scratch/uncommitted-config"
new_repo "$repo"
mkdir -p "$repo/.cargo"
printf '[build]\nrustflags = ["-A", "warnings"]\n' > "$repo/.cargo/config.toml"
printf 'fn tracked() { let CHA27_UNCOMMITTED_SUPPRESSED = 1; }\n' > "$repo/tracked.rs"
git -C "$repo" add tracked.rs
if (
    cd "$repo"
    HOOK_TEST_WARN_ON_MARKER='CHA27_UNCOMMITTED_SUPPRESSED' bash "$hook" > "$scratch/uncommitted-config.log" 2>&1
); then
    fail 'an untracked .cargo/config.toml silenced a warning in the commit'
else
    pass 'an untracked .cargo/config.toml is not honoured'
fi

# A rustflags the hook cannot reproduce exactly must be refused, not dropped:
# RUSTFLAGS is set for the lint run, so anything missed would be silently
# ignored and the hook would lint content that differs from the commit's build.
repo="$scratch/target-config"
new_repo "$repo"
mkdir -p "$repo/.cargo"
printf "[target.'cfg(all())']\nrustflags = ['-A', 'warnings']\n" > "$repo/.cargo/config.toml"
git -C "$repo" add .cargo/config.toml
if (cd "$repo" && bash "$hook" > "$scratch/target-config.log" 2>&1); then
    fail 'a committed [target.*] rustflags was silently ignored'
elif ! grep -q 'sets rustflags for a specific target' "$scratch/target-config.log"; then
    fail 'the [target.*] rustflags case failed for the wrong reason'
    cat "$scratch/target-config.log" >&2
else
    pass 'committed [target.*] rustflags are refused rather than ignored'
fi

# A commented-out rustflags is not a setting, so it must not be applied: doing
# so would lint with a flag the commit does not have.
repo="$scratch/commented-rustflags"
new_repo "$repo"
mkdir -p "$repo/.cargo"
printf '[build]\n# rustflags = ["-A", "warnings"]\n' > "$repo/.cargo/config.toml"
printf 'fn tracked() { let CHA27_COMMENTED_WARNING = 1; }\n' > "$repo/tracked.rs"
git -C "$repo" add .cargo/config.toml tracked.rs
if (
    cd "$repo"
    HOOK_TEST_WARN_ON_MARKER='CHA27_COMMENTED_WARNING' bash "$hook" > "$scratch/commented-rustflags.log" 2>&1
); then
    fail 'a commented-out rustflags was applied'
else
    pass 'a commented-out rustflags is ignored'
fi

# TOML literal strings ('...') are valid too, so a commit using them must be
# linted with its own flags rather than refused or silently stripped.
repo="$scratch/literal-string-rustflags"
new_repo "$repo"
mkdir -p "$repo/.cargo"
printf "[build]\nrustflags = ['-A', 'warnings']\n" > "$repo/.cargo/config.toml"
printf 'fn tracked() { let CHA27_LITERAL_SUPPRESSED = 1; }\n' > "$repo/tracked.rs"
git -C "$repo" add .cargo/config.toml tracked.rs
if (
    cd "$repo"
    HOOK_TEST_WARN_ON_MARKER='CHA27_LITERAL_SUPPRESSED' bash "$hook" > "$scratch/literal-string-rustflags.log" 2>&1
); then
    pass 'rustflags written as TOML literal strings are honoured'
else
    fail 'a commit was blocked by a warning its own literal-string rustflags silence'
    cat "$scratch/literal-string-rustflags.log" >&2
fi

# A comment that merely mentions rustflags is not a setting either: refusing on
# prose would block a commit whose own content is clean.
repo="$scratch/prose-rustflags"
new_repo "$repo"
mkdir -p "$repo/.cargo"
printf '# Do not add rustflags here; CI owns compiler flags.\n[build]\njobs = 2\n' > "$repo/.cargo/config.toml"
printf 'fn staged_clean() {}\n' > "$repo/clean.rs"
git -C "$repo" add .cargo/config.toml clean.rs
if (cd "$repo" && bash "$hook" > "$scratch/prose-rustflags.log" 2>&1); then
    pass 'a comment that mentions rustflags does not block a clean commit'
else
    fail 'a comment mentioning rustflags blocked a clean commit'
    cat "$scratch/prose-rustflags.log" >&2
fi

# A ']' inside a quoted element must not end the array early, or the flags
# after it would be silently dropped.
repo="$scratch/bracket-in-string-rustflags"
new_repo "$repo"
mkdir -p "$repo/.cargo"
printf '[build]\nrustflags = ["--cfg", "cha27_bracket", "-A", "warnings"]\n' > "$repo/.cargo/config.toml"
printf 'fn tracked() { let CHA27_BRACKET_SUPPRESSED = 1; }\n' > "$repo/tracked.rs"
git -C "$repo" add .cargo/config.toml tracked.rs
if (
    cd "$repo"
    HOOK_TEST_WARN_ON_MARKER='CHA27_BRACKET_SUPPRESSED' bash "$hook" > "$scratch/bracket-in-string-rustflags.log" 2>&1
); then
    pass 'flags after a quoted element are still applied'
else
    fail 'a flag list was truncated at a quoted element'
    cat "$scratch/bracket-in-string-rustflags.log" >&2
fi

# --- A stale lock cannot be "stolen" from a live holder ------------------
# The reclaim used to rename the lock aside and restore it with mv; renaming a
# directory onto an existing directory nests instead of failing, so a live
# lock could be taken over and two hooks would lint at once. With a lock file
# and re-read verification, a live holder keeps it.
repo="$scratch/live-lock-holder"
new_repo "$repo"
printf 'fn staged_clean() {}\n' > "$repo/clean.rs"
git -C "$repo" add clean.rs
live_marker="$scratch/live-lock-holder.marker"
live_release="$scratch/live-lock-holder.release"
live_log="$scratch/live-lock-holder.log"
(
    cd "$repo" || exit 1
    HOOK_TEST_HANG_MARKER="$live_marker" HOOK_TEST_RELEASE_FILE="$live_release" \
        bash "$hook" > "$live_log" 2>&1 &
    holder=$!
    for _ in $(seq 1 100); do
        [ -e "$live_marker" ] && break
        sleep 0.1
    done
    if [ ! -e "$live_marker" ]; then
        echo 'ERROR: holder never reached cargo' >&2
        kill -KILL "$holder" 2>/dev/null || true
        exit 1
    fi
    holder_pid=$(cat "$repo/.git/chartml-pre-commit/lock" 2>/dev/null || true)
    # A second hook must not take the lock away from the live holder.
    bash "$hook" > "$scratch/live-lock-holder.second.log" 2>&1 &
    second=$!
    for _ in $(seq 1 40); do
        kill -0 "$second" 2>/dev/null || break
        sleep 0.1
    done
    still_owner=$(cat "$repo/.git/chartml-pre-commit/lock" 2>/dev/null || true)
    printf '%s|%s\n' "$holder_pid" "$still_owner" > "$scratch/live-lock-holder.owners"
    : > "$live_release"
    wait "$holder" || echo 'HOLDER_FAILED' > "$scratch/live-lock-holder.result"
    wait "$second" || echo 'SECOND_FAILED' > "$scratch/live-lock-holder.result2"
) || true
if [ -e "$scratch/live-lock-holder.result" ]; then
    fail 'a second hook made the live holder fail'
    tail -5 "$live_log" >&2
elif [ -e "$scratch/live-lock-holder.result2" ]; then
    fail 'a second hook failed against a live holder'
    tail -5 "$scratch/live-lock-holder.second.log" >&2
else
    pass 'a live lock holder is not displaced by a second hook'
fi
if [ -e "$scratch/live-lock-holder.owners" ]; then
    owners=$(cat "$scratch/live-lock-holder.owners")
    holder_pid=${owners%%|*}
    still_owner=${owners#*|}
    if [ -n "$holder_pid" ] && [ "$holder_pid" = "$still_owner" ]; then
        pass 'the live holder still owns the lock while the second hook waits'
    else
        fail "the lock changed hands while its holder was live ($holder_pid -> $still_owner)"
    fi
fi

# A "]" or a quote inside a comment is prose. Neither may end the array (which
# would drop real flags) nor become a flag (which would apply one that the
# commit does not have).
repo="$scratch/comment-bracket-rustflags"
new_repo "$repo"
mkdir -p "$repo/.cargo"
printf '[build]\nrustflags = [\n  "--cfg", "cha27_keep", # [CHA-27] keep this\n  "-A", "warnings",\n]\n' > "$repo/.cargo/config.toml"
printf 'fn tracked() { let CHA27_COMMENT_BRACKET = 1; }\n' > "$repo/tracked.rs"
git -C "$repo" add .cargo/config.toml tracked.rs
if (
    cd "$repo"
    HOOK_TEST_WARN_ON_MARKER='CHA27_COMMENT_BRACKET' bash "$hook" > "$scratch/comment-bracket-rustflags.log" 2>&1
); then
    pass 'a "]" in a trailing comment does not drop later flags'
else
    fail 'a "]" in a trailing comment truncated the flag list'
    cat "$scratch/comment-bracket-rustflags.log" >&2
fi

repo="$scratch/comment-quote-rustflags"
new_repo "$repo"
mkdir -p "$repo/.cargo"
printf '[build]\nrustflags = [\n  "-A", # do not add "extra" flags\n  "warnings",\n]\n' > "$repo/.cargo/config.toml"
printf 'fn tracked() { let CHA27_COMMENT_QUOTE = 1; }\n' > "$repo/tracked.rs"
git -C "$repo" add .cargo/config.toml tracked.rs
if (
    cd "$repo"
    HOOK_TEST_WARN_ON_MARKER='CHA27_COMMENT_QUOTE' bash "$hook" > "$scratch/comment-quote-rustflags.log" 2>&1
); then
    pass 'a quote in a trailing comment does not corrupt the flags'
else
    fail 'a quote in a trailing comment corrupted the flag list'
    cat "$scratch/comment-quote-rustflags.log" >&2
fi

# A "]" inside a quoted element is a value, not the end of the array.
repo="$scratch/bracket-in-value-rustflags"
new_repo "$repo"
mkdir -p "$repo/.cargo"
printf '[build]\nrustflags = [\n  "-C",\n  "link-arg=-Wl,z=[anchor]",\n  "-A",\n  "warnings",\n]\n' > "$repo/.cargo/config.toml"
printf 'fn tracked() { let CHA27_BRACKET_VALUE = 1; }\n' > "$repo/tracked.rs"
git -C "$repo" add .cargo/config.toml tracked.rs
if (
    cd "$repo"
    HOOK_TEST_WARN_ON_MARKER='CHA27_BRACKET_VALUE' bash "$hook" > "$scratch/bracket-in-value-rustflags.log" 2>&1
); then
    pass 'a "]" inside a quoted value does not end the array'
else
    fail 'a "]" inside a quoted value truncated the flag list'
    cat "$scratch/bracket-in-value-rustflags.log" >&2
fi

if [ "$failures" -ne 0 ]; then
    echo "ERROR: $failures pre-commit test(s) failed" >&2
    exit 1
fi
echo 'Pre-commit fixtures passed.'
