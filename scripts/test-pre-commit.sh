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
trap 'rm -rf "$scratch"' EXIT

failures=0
pass() { echo "PASS: $*"; }
fail() {
    echo "FAIL: $*" >&2
    failures=$((failures + 1))
}

# --- Fixture shared by every scenario -------------------------------------
# A fake cargo on PATH records the directory it linted, so the tests can prove
# the hook lints the staged snapshot rather than the working tree, and can
# simulate warnings, failures and hangs.
setup_bin() {
    mkdir -p "$scratch/bin"
    cat > "$scratch/bin/cargo" <<'CARGO'
#!/usr/bin/env bash
printf '%s\n' "$PWD" >> "${HOOK_TEST_PWD_LOG:-/dev/null}"
if [ -n "${HOOK_TEST_PID_FILE:-}" ]; then
    printf '%s\n' "$$" > "$HOOK_TEST_PID_FILE"
fi
printf '%s\n' "${HOOK_TEST_OUTPUT:-}"
# Emit a warning when the linted content itself contains a marker, so tests
# can prove the verdict follows the linted content and nothing else.
if [ -n "${HOOK_TEST_WARN_ON_MARKER:-}" ] && grep -rqF -- "$HOOK_TEST_WARN_ON_MARKER" .; then
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
    printf 'fn tracked() {}\n' > "$repo/tracked.rs"
    printf 'fn staged() {}\n' > "$repo/staged.rs"
    git -C "$repo" add tracked.rs staged.rs
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

# --- Interrupt: the working tree survives a killed hook --------------------
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
# A terminal delivers Ctrl-C to the hook and cargo together, so the test puts
# the hook in its own process group (job control) and signals that group.
(
    set -m
    cd "$repo" || exit 1
    HOOK_TEST_HANG_MARKER="$marker" HOOK_TEST_HANG_SECONDS=30 HOOK_TEST_PID_FILE="$cargo_pid_file" \
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
    kill -INT -- -"$hook_pid" 2>/dev/null || true
    wait "$hook_pid"
    printf '%s\n' "$?" > "$interrupt_status"
    # Nothing should survive the interrupt; make sure of it before moving on.
    kill -KILL -- -"$hook_pid" 2>/dev/null || true
) || true
if [ -e "$marker" ] && grep -qx '130' "$interrupt_status" 2>/dev/null; then
    pass 'hook was interrupted while cargo was linting'
else
    fail "hook was not interrupted while linting (status: $(cat "$interrupt_status" 2>/dev/null || echo none))"
fi
snapshot_worktree "$repo" "$scratch/interrupt.after"
if diff -u "$scratch/interrupt.before" "$scratch/interrupt.after"; then
    pass 'interrupted hook left every unstaged and untracked change untouched'
else
    fail 'interrupted hook altered the working tree'
fi
if [ -e "$repo/.git/chartml-pre-commit/lock" ]; then
    fail 'interrupted hook left its snapshot lock behind'
else
    pass 'interrupted hook released its snapshot lock'
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
    fail 'hook signalled on its own left its snapshot lock behind'
else
    pass 'hook signalled on its own released its snapshot lock'
fi

# --- Snapshot exactness: the hook lints the index, not the working tree ---
repo="$scratch/snapshot-content"
new_repo "$repo"
pwd_log="$scratch/snapshot-content.pwd"
(
    cd "$repo"
    HOOK_TEST_PWD_LOG="$pwd_log" bash "$hook" >/dev/null 2>&1
) || fail 'hook failed while recording the linted directory'
linted=$(cat "$pwd_log")
expected=$(cd -- "$repo" && pwd -P)/.git/chartml-pre-commit/tree
case "$linted" in
    "$expected"|"$(cd -- "$expected" && pwd -P)") pass "hook linted the snapshot, not the working tree" ;;
    *) fail "hook linted unexpected directory: $linted (expected $expected)" ;;
esac

# A file deleted from the index must be gone from the snapshot, and staged
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
snapshot="$repo/.git/chartml-pre-commit/tree"
if [ -e "$snapshot/stale.rs" ]; then
    fail 'file deleted from the index survived in the snapshot'
else
    pass 'file deleted from the index is absent from the snapshot'
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
elif grep -q 'does not match the index content' "$scratch/snapshot-mismatch.log"; then
    pass 'snapshot that differs from the commit is refused'
else
    fail 'snapshot mismatch was rejected for the wrong reason'
    cat "$scratch/snapshot-mismatch.log" >&2
fi

# --- Linked worktrees keep separate snapshots -----------------------------
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
# clippy log, so a clean commit failed for an unrelated reason.
repo="$scratch/second-hook-cleanup"
new_repo "$repo"
printf 'base\n' > "$repo/held.rs"
git -C "$repo" add held.rs
git -C "$repo" commit -qm 'add the file both branches change'
seed_dirty_working_tree "$repo"
printf 'fn staged_clean() {}\n' > "$repo/clean.rs"
git -C "$repo" add clean.rs
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
    # Produce a genuine merge conflict so the second hook exits at the
    # unresolved-index check, which happens before it takes the lock.
    git stash -q --include-untracked
    git checkout -q -b conflicting-side
    printf 'theirs\n' > held.rs
    git commit -qam theirs
    git checkout -q -
    printf 'ours\n' > held.rs
    git commit -qam ours
    git merge conflicting-side >/dev/null 2>&1 || true
    bash "$hook" >/dev/null 2>&1 || true
    # Only now let the first hook finish linting.
    : > "$first_release"
    if wait "$first_pid"; then
        :
    else
        echo 'FIRST_FAILED' > "$scratch/second-hook-cleanup.result"
    fi
) || true
if [ -e "$scratch/second-hook-cleanup.result" ]; then
    fail 'a second hook that exited early broke the running hook'
    tail -5 "$first_log" >&2
elif grep -q '✅ Clippy clean' "$first_log"; then
    pass 'a second hook that exits early leaves the running hook clean'
else
    fail 'the running hook did not finish cleanly'
    tail -5 "$first_log" >&2
fi

if [ "$failures" -ne 0 ]; then
    echo "ERROR: $failures pre-commit test(s) failed" >&2
    exit 1
fi
echo 'Pre-commit fixtures passed.'
