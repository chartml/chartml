# Ticket worktree preservation

These helpers support the shared `backlog-fast` stranded-ticket workflow. They do
not decide that an agent is abandoned, change Trakkt status, delete files, or clean
up completed worktrees. Confirm abandonment and acquire the workflow's ticket
reservation before invoking a writer. Preserve uncertain claims.

Requirements: Python 3 with POSIX `fcntl` locking, Git 2.31+, Bash, and authenticated
`gh`. The team comes from `.claude/team.md` (`Team key:` or `team_key:`); GitHub
identity comes from the selected Git remote URL, rather than stale team metadata.

```sh
scripts/check-ticket-in-flight.sh CHA-17
scripts/check-ticket-in-flight.sh CHA-17 --self feature/cha-17-work
scripts/mark-worktree-stranded.sh CHA-17 --worktree /path/to/linked/tree --note 'Evidence recorded in ticket'
scripts/mark-branch-stranded.sh CHA-17 --branch feature/cha-17-work --note 'Evidence recorded in ticket'
```

All helpers accept `--remote origin` (also a legacy positional remote). The checker
accepts repeated `--ignore-branch NAME`; exclusions must be justified by the
caller, and there is no automatic current-branch exclusion. Branch matching
supports `cha-17`, `cha17`, and similar team conventions without matching ticket
170 or a different team prefix.

Checker exit codes: 0 clear, 1 existing work, 2 usage/configuration mismatch, 3
incomplete evidence. Only 0 authorizes selection. It examines all paginated PR
head branches, remote branches, registered local worktrees and local branches.
Closed and merged PRs remain evidence. A local `STRANDED.md` only releases that
worktree and its local branch; it cannot hide remote branches or PRs. Preserved
`stranded/` refs are printed for salvage.

Writers reject primary, foreign, detached and default-branch worktrees, mismatched
ticket branches, conflicting/symlink markers, and PR-backed branches. The marker
preserves existing files and index. A remote branch must be preserved atomically:
the server creates `stranded/BRANCH` and deletes `BRANCH` together, guarded by
expected-SHA leases. Remotes without atomic push support fail closed. If both
remote names already exist, the helper refuses ambiguous recovery, including
same-SHA copies; inspect ownership and both refs before resolving that condition.
A checked-out branch needs its matching marker and remains checked out; only a
branch without a registered checkout is renamed locally. Other ticket branches
are never swept. A local-only branch has no remote archive target and is refused;
preserve its worktree and commits for explicit salvage. Writers recheck local ownership before mutation; the surrounding
workflow reservation remains required because unrelated Git clients do not obey
these helpers' advisory locks.

Tests use disposable real Git repositories and a stub GitHub CLI, never live
branches or ticket state:

```sh
python3 -m unittest discover -s scripts/tests -p test_ticket_worktrees.py -v
```
