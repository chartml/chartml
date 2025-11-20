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

- Node.js 18+
- pnpm 8+ (recommended for monorepo management)
- Git

### Clone and Install

```bash
# Clone the repository
git clone https://github.com/chartml/chartml.git
cd chartml

# Install dependencies (all packages)
pnpm install

# Build all packages
pnpm build
```

### Development Workflow

```bash
# Build all packages in watch mode
pnpm dev

# Run tests
pnpm test

# Start documentation site
cd docs && npm run docs:dev
```

## Project Structure

```
chartml/
├── packages/
│   ├── core/              # @chartml/core - Core library
│   ├── chart-pie/         # @chartml/chart-pie - Pie chart plugin
│   ├── chart-scatter/     # @chartml/chart-scatter - Scatter plot plugin
│   ├── chart-metric/      # @chartml/chart-metric - Metric card plugin
│   ├── react/             # @chartml/react - React wrapper
│   ├── markdown-it/       # @chartml/markdown-it - Markdown-it plugin
│   ├── markdown-react/    # @chartml/markdown-react - React-markdown plugin
│   └── markdown-common/   # @chartml/markdown-common - Shared utilities
├── docs/                  # VitePress documentation site
└── README.md
```

### Package Dependencies

```
core (base package)
  ├── chart-pie (plugin)
  ├── chart-scatter (plugin)
  ├── chart-metric (plugin)
  ├── react (framework integration)
  └── markdown-common (shared utilities)
      ├── markdown-react (framework integration)
      └── markdown-it (framework integration)
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
# Run tests for all packages
pnpm test

# Run tests for specific package
cd packages/core
npm test

# Build to ensure no errors
pnpm build
```

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
pnpm test

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
