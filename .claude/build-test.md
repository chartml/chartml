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
