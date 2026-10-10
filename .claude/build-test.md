## Worktree Setup
- Not required — ChartML does not use worktrees for backlog work
- Work directly in /home/jason/repos/chartml on a feature branch

## Build Commands
- Quick check: cargo check --workspace
- Full build for browser testing: cd demo && trunk build
- Test gallery: cargo run -p chartml-test-runner -- --gallery
- Serve for testing: trunk serve --address 0.0.0.0 --port 8642 &>/tmp/chartml-demo.log &

## Test Commands
- Rust unit tests: cargo test --workspace
- Golden baseline tests: cargo run -p chartml-test-runner
- npm/WASM tests: npm run test

## Verification
- Run cargo check --workspace before every commit
- Run cargo test --workspace to verify no regressions
- For chart rendering changes: run chartml-test-runner to verify golden baselines
- For WASM/npm changes: run npm run test
- Post verification report as Trakkt ticket comment

## Stranded ticket reclamation tooling

Run this repository's scripts from its checkout or ticket worktree. Requirements:
Python 3, Git, and authenticated `gh` access to the actual `origin` repository.
Discover the default branch from `origin`; do not assume it is `main`.

Use `scripts/check-ticket-in-flight.sh TICKET` before claiming work. Only exit 0
permits claiming; reported existing work or incomplete checks must be investigated.

`scripts/mark-worktree-stranded.sh` and `scripts/mark-branch-stranded.sh` are
preservation tools, not abandonment detectors. Run them only after the canonical
`backlog-fast` skill's conservative abandonment checks and per-ticket reservation,
then recheck ownership before writing. An existing PR prevents reclamation.
Preserve uncommitted files, unpublished commits, other agents' claims and validation
evidence. These tools do not remove worktrees or update Trakkt ticket statuses.
Do not use elapsed time alone as proof that an agent has abandoned a ticket.

For tooling-only changes, run:

```bash
python3 -m unittest discover -s scripts/tests -p test_ticket_worktrees.py -v
bash -n scripts/check-ticket-in-flight.sh scripts/mark-worktree-stranded.sh scripts/mark-branch-stranded.sh
```

Product compilation is not needed to validate changes limited to these
Git/worktree tools.
