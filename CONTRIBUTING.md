# Contributing to ChartML

Thank you for your interest in contributing to ChartML! This document provides guidelines and instructions for contributing.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Project Structure](#project-structure)
- [Making Changes](#making-changes)
- [Testing](#testing)
- [Submitting Changes](#submitting-changes)
- [Coding Standards](#coding-standards)
- [Creating Plugins](#creating-plugins)

## Code of Conduct

This project adheres to a Code of Conduct that all contributors are expected to follow. Please read [CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md) before contributing.

## Getting Started

### Ways to Contribute

- **Report bugs** - Found a bug? [Open an issue](https://github.com/chartml/chartml/issues/new)
- **Suggest features** - Have an idea? [Start a discussion](https://github.com/chartml/chartml/discussions)
- **Improve documentation** - Fix typos, clarify examples, add guides
- **Write code** - Fix bugs, implement features, create plugins
- **Create examples** - Share your ChartML dashboards and use cases

### First Time Contributors

Look for issues labeled [`good first issue`](https://github.com/chartml/chartml/labels/good%20first%20issue) or [`help wanted`](https://github.com/chartml/chartml/labels/help%20wanted).

## Development Setup

### Prerequisites

- Node.js 18+ (CI uses 22) with npm 9+. The repo is an npm workspace with a committed `package-lock.json`; do not use pnpm or yarn.
- Rust (stable) with the `wasm32-unknown-unknown` target: `rustup target add wasm32-unknown-unknown`
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/), which the package build scripts call
- Git

### Clone and Install

```bash
# Clone the repository
git clone https://github.com/chartml/chartml.git
cd chartml

# Install dependencies (all packages) exactly as pinned in package-lock.json
npm ci

# Build all packages (WASM via packages/*/build.sh, then TypeScript)
npm run build
```

### Enable the pre-commit hook

The repo's hook rejects new `#[allow(...)]` lint suppressions and compiler/Clippy warnings, as CI does. Enable it once per clone:

```bash
git config core.hooksPath .githooks
```

### Development Workflow

```bash
# Run JS/WASM tests
npm test

# Start documentation site
cd docs && npm run docs:dev
```

### Checks CI runs

`.github/workflows/ci.yml` is the source of truth. If anything below differs from it, ci.yml wins; please fix this section. Each command is listed under the CI job that runs it. Run them from the repository root.

**Lint Suppression Policy** (pull requests only)

```bash
bash scripts/test-pre-commit.sh
python3 scripts/tests/test_check_md_fences.py
python3 scripts/check-md-fences.py docs
```

The job also rejects any new `#[allow(` in `.rs` files and any new `= "allow"` in `Cargo.toml` files in your diff.

**Clippy**

```bash
cargo clippy --workspace --exclude chartml-wasm --exclude chartml-wasm-datafusion --all-targets -- -D warnings
cargo clippy -p chartml-core --all-targets --target wasm32-unknown-unknown --features wasm-indexeddb -- -D warnings
cargo clippy -p chartml-leptos -p chartml-wasm -p chartml-wasm-datafusion --all-targets --target wasm32-unknown-unknown -- -D warnings
```

**Rust Tests**

```bash
cargo test --workspace --exclude chartml-wasm --exclude chartml-wasm-datafusion
```

The same job then runs four browser (`wasm-bindgen-test`) suites in headless Chrome; see [Running the browser tests locally](#running-the-browser-tests-locally).

**WASM Build**

```bash
npm ci
bash packages/core/build.sh
bash packages/datafusion/build.sh
bash scripts/check-npm-pack.sh
npm run docs:build
python3 scripts/tests/test_check_docs_chart_errors.py
python3 scripts/check-docs-chart-errors.py
```

### Running the browser tests locally

The `wasm_bindgen_test` suites only run in a real browser. CI pins a runner, Chrome and chromedriver; locally you need the same pieces:

1. A `wasm-bindgen-test-runner` that exactly matches the `wasm-bindgen` version in `Cargo.lock`:

   ```bash
   WASM_BINDGEN_VERSION="$(cargo metadata --locked --format-version 1 \
     | python3 -c 'import json, sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"] == "wasm-bindgen"))')"
   cargo install wasm-bindgen-cli --version "$WASM_BINDGEN_VERSION" --locked
   ```

2. [Chrome for Testing](https://googlechromelabs.github.io/chrome-for-testing/) and its matching chromedriver. CI pins the version in `CHROME_VERSION` in ci.yml; pick the `mac-arm64`, `mac-x64` or `linux64` download for your machine.

3. A WebDriver config that starts Chrome headless. Point `binary` at your Chrome for Testing executable. The last two flags (not in CI) stop Chrome asking for keychain access on macOS.

   ```bash
   cat > /tmp/chartml-webdriver.json <<'JSON'
   {
     "goog:chromeOptions": {
       "binary": "/path/to/chrome-for-testing/chrome",
       "args": ["--headless=new", "--no-sandbox", "--disable-dev-shm-usage", "--disable-gpu", "--use-mock-keychain", "--password-store=basic"]
     }
   }
   JSON
   ```

4. Run the suites with the same environment as CI. A suite with no tests exits 0, so check that each reports the expected number of passing tests (the `run_suite` lines in ci.yml give the count for each):

   ```bash
   export CHROMEDRIVER=/path/to/chromedriver
   export CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner
   export WASM_BINDGEN_TEST_TIMEOUT=120
   export WASM_BINDGEN_TEST_DRIVER_TIMEOUT=30
   export WASM_BINDGEN_TEST_WEBDRIVER_JSON=/tmp/chartml-webdriver.json

   cargo test --target wasm32-unknown-unknown -p chartml-wasm --test web
   cargo test --target wasm32-unknown-unknown -p chartml-leptos --test table_layout_test
   cargo test --target wasm32-unknown-unknown -p chartml-leptos --test chart_provider_test
   cargo test --target wasm32-unknown-unknown -p chartml-core --features wasm-indexeddb --test indexeddb_test
   ```

   The counts change when a suite gains or loses a test, so they are not repeated here.

## Project Structure

```
chartml/
├── crates/                # Rust crates (the rendering engine)
│   ├── chartml-core/
│   ├── chartml-forecast/
│   ├── chartml-chart-cartesian/ chartml-chart-metric/ chartml-chart-pie/
│   │   chartml-chart-scatter/ chartml-chart-table/
│   ├── chartml-datafusion/
│   ├── chartml-render/    # server-side SVG rendering
│   ├── chartml-leptos/
│   ├── chartml-wasm/ chartml-wasm-datafusion/   # WASM bindings
│   └── chartml-test-runner/                     # golden-baseline runner
├── packages/              # npm workspaces
│   ├── core/              # @chartml/core - WASM bindings + JS/TS wrapper
│   ├── datafusion/        # @chartml/datafusion - DataFusion WASM add-on
│   ├── react/             # @chartml/react - React wrapper
│   ├── markdown-it/       # @chartml/markdown-it - markdown-it plugin
│   └── markdown-react/    # @chartml/markdown-react - react-markdown plugin
├── demo/                  # Leptos demo app (trunk build)
├── docs/                  # VitePress documentation site
├── tests/charts/          # YAML chart specs
├── test-output/golden/    # golden SVG baselines
└── README.md
```

### Package Dependencies

```
@chartml/core (base package)
  ├── @chartml/datafusion (peer: core)
  ├── @chartml/react (peer: core)
  └── @chartml/markdown-it (peer: core)
      └── @chartml/markdown-react (peer: core, markdown-it)
```

## Making Changes

### 1. Create a Branch

```bash
git checkout -b feature/your-feature-name
# or
git checkout -b fix/bug-description
```

### 2. Make Your Changes

- Follow the [coding standards](#coding-standards)
- Add tests for new functionality
- Update documentation as needed
- Keep commits atomic and well-described

### 3. Test Your Changes

```bash
# Run JS tests for all packages
npm test

# Run tests for specific package
cd packages/core
npm test

# Build to ensure no errors
npm run build
```

Before opening a pull request, run the commands under [Checks CI runs](#checks-ci-runs) that cover what you changed.

### 4. Update Documentation

If your change affects user-facing behavior:

- Update relevant package README
- Update documentation site (`docs/docs/`)
- Add examples to `docs/docs/examples.md`
- Update API reference if needed

## Testing

### Running Tests

```bash
# All packages
npm test

# Specific package
cd packages/core && npm test

# Watch mode
cd packages/core && npm test -- --watch
```

### Writing Tests

We use Vitest for testing. Place tests in `__tests__` directories or alongside source files with `.test.js` suffix.

```javascript
import { describe, it, expect } from 'vitest';
import { ChartML } from '../src/index.js';

describe('ChartML', () => {
  it('should render a bar chart', async () => {
    const chartml = new ChartML();
    const spec = `
      type: chart
      version: 1
      data: [...]
      visualize: { type: bar }
    `;

    const container = document.createElement('div');
    await chartml.render(spec, container);

    expect(container.querySelector('svg')).toBeTruthy();
  });
});
```

### Test Coverage

Aim for:
- **Core functionality**: 80%+ coverage
- **Public API**: 100% coverage
- **Edge cases**: Document with tests

## Submitting Changes

### Pull Request Process

1. **Update your fork**

```bash
git remote add upstream https://github.com/chartml/chartml.git
git fetch upstream
git rebase upstream/main
```

2. **Push your changes**

```bash
git push origin feature/your-feature-name
```

3. **Open a Pull Request**

- Use a clear, descriptive title
- Reference related issues (e.g., "Fixes #123")
- Describe what changed and why
- Include screenshots for UI changes
- List any breaking changes

### Pull Request Template

```markdown
## Description
Brief description of changes

## Related Issues
Fixes #123

## Changes Made
- Added X feature
- Fixed Y bug
- Updated Z documentation

## Testing
- [ ] Tests added/updated
- [ ] All tests passing
- [ ] Documentation updated

## Screenshots (if applicable)
[Add screenshots here]

## Breaking Changes
List any breaking changes and migration path
```

### Review Process

- Maintainers will review your PR
- Address feedback with new commits
- Once approved, maintainers will merge
- Your contribution will be included in the next release!

## Coding Standards

### JavaScript Style

- **ES6+ syntax** - Use modern JavaScript features
- **No semicolons** - Follow existing style
- **2-space indentation** - Consistent with project
- **Descriptive names** - Clear variable and function names

### Code Organization

```javascript
// Good
function calculateTotal(items) {
  return items.reduce((sum, item) => sum + item.price, 0);
}

// Bad
function calc(arr) {
  let t = 0;
  for (let i = 0; i < arr.length; i++) t += arr[i].price;
  return t;
}
```

### Documentation

- **JSDoc comments** for all public APIs
- **Inline comments** for complex logic
- **README updates** for new features

```javascript
/**
 * Render a ChartML specification into a container
 *
 * @param {string|object} spec - ChartML YAML string or object
 * @param {HTMLElement} container - DOM element to render into
 * @returns {Promise<Chart>} Chart instance
 *
 * @example
 * const chartml = new ChartML();
 * await chartml.render(spec, container);
 */
async render(spec, container) {
  // Implementation
}
```

### Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```bash
feat: add pie chart renderer
fix: resolve axis label collision
docs: update getting started guide
test: add scatter plot tests
chore: upgrade dependencies
```

**Types:**
- `feat` - New feature
- `fix` - Bug fix
- `docs` - Documentation only
- `style` - Code style changes (formatting)
- `refactor` - Code refactoring
- `test` - Adding or updating tests
- `chore` - Maintenance tasks

## Creating Plugins

### Chart Renderer Plugin

Create a new chart type by implementing the renderer interface:

```javascript
import { globalRegistry } from '@chartml/core';

export function createMyChartRenderer() {
  return function renderMyChart(container, data, config) {
    // 1. Clear container
    container.innerHTML = '';

    // 2. Create visualization using D3
    const svg = d3.create('svg')
      .attr('width', config.width)
      .attr('height', config.height);

    // 3. Add your chart implementation
    // ...

    // 4. Append to container
    container.appendChild(svg.node());
  };
}

// Auto-register on import
globalRegistry.registerChartRenderer('myChart', createMyChartRenderer());
```

### Data Source Plugin

Add support for custom data sources:

```javascript
import { globalRegistry } from '@chartml/core';

globalRegistry.registerDataSource('postgresql', async (spec) => {
  const { host, database, query } = spec;

  // Connect to PostgreSQL
  const client = await connectToPostgres({ host, database });

  // Execute query
  const result = await client.query(query);

  // Return rows array
  return result.rows;
});
```

### Publishing Plugins

Community plugins should:
- Use naming pattern: `chartml-plugin-{name}`
- Include comprehensive README
- Add examples and tests
- List `@chartml/core` as peer dependency

## Questions?

- **Documentation**: https://chartml.org
- **Discussions**: https://github.com/chartml/chartml/discussions
- **Issues**: https://github.com/chartml/chartml/issues

## License

By contributing to ChartML, you agree that your contributions will be licensed under the MIT License.

---

**Thank you for contributing to ChartML!** 🎉
