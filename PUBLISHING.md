# Publishing ChartML Packages to NPM

This document explains how to publish ChartML packages to NPM using the automated GitHub Actions workflow.

## Overview

ChartML uses an automated GitHub Actions workflow to publish packages to NPM. The workflow:

1. Detects which packages have version changes (compares to git tags)
2. Builds all packages in the workspace
3. Publishes changed packages in dependency order
4. Creates git tags for published versions
5. Creates GitHub releases with changelogs

## Prerequisites

### NPM Token Setup (One-Time)

The workflow requires an NPM authentication token to be configured as a GitHub Secret:

1. **Generate NPM token** (if you don't have one):
   - Log in to [npmjs.com](https://www.npmjs.com)
   - Go to Account Settings → Access Tokens
   - Click "Generate New Token" → "Automation"
   - Copy the token (starts with `npm_...`)

2. **Add token to GitHub Secrets**:
   - Go to: https://github.com/jasadams/chartml/settings/secrets/actions
   - Click "New repository secret"
   - Name: `NPM_TOKEN`
   - Value: Paste your NPM token
   - Click "Add secret"

The workflow will use this token to authenticate with NPM during publishing.

## Publishing Process

### Step 1: Bump Package Versions

Decide which packages need publishing and bump their versions:

```bash
# Bump a single package
cd /home/jason/repos/chartml/packages/core
npm version patch  # 1.4.0 → 1.4.1 (or 'minor' or 'major')

# Or bump multiple packages
cd /home/jason/repos/chartml
npm version patch --workspaces  # Bumps all workspace packages
```

### Step 2: Commit Version Changes

```bash
git add .
git commit -m "Bump versions for npm publish"
git push origin main
```

### Step 3: Trigger GitHub Actions Workflow

1. Go to: https://github.com/jasadams/chartml/actions
2. Select "Publish to NPM" workflow in the left sidebar
3. Click "Run workflow" button (top right)
4. Configure options:
   - **dry_run**: Select `true` for testing, `false` for actual publish
5. Click "Run workflow"

### Step 4: Monitor Workflow Execution

The workflow will:

- **Detect changed packages**: Compares package.json versions with git tags
- **Build packages**: Runs `npm run build` for all packages
- **Fix dependencies**: Temporarily replaces `file:../` with NPM versions
- **Publish**: Pushes packages to NPM in dependency order
- **Create tags**: Tags releases as `@chartml/package@version`
- **Create releases**: Creates GitHub releases with notes

### Step 5: Verify Publication

After the workflow completes:

```bash
# Check NPM
npm view @chartml/core version

# Check GitHub tags
git fetch --tags
git tag -l

# Check GitHub releases
# Visit: https://github.com/jasadams/chartml/releases
```

## Dry Run Mode

**Always test with dry run first!**

When `dry_run: true`:
- Detects which packages would be published
- Builds packages to verify no build errors
- Validates package contents
- **Does NOT** actually publish to NPM
- **Does NOT** create git tags or releases

This lets you verify everything works before publishing.

## Dependency Order

Packages are automatically published in the correct dependency order:

1. **@chartml/core** (no dependencies)
2. **@chartml/chart-pie**, **@chartml/chart-scatter**, **@chartml/chart-metric** (depend on core)
3. **@chartml/markdown-common** (depends on core)
4. **@chartml/react** (depends on core + chart-*)
5. **@chartml/markdown-it**, **@chartml/markdown-react** (depend on markdown-common, core, chart-*)

## Troubleshooting

### Workflow fails: "No packages need publishing"

**Cause**: All package versions already have git tags.

**Solution**: Bump the version in package.json and push:
```bash
cd packages/core
npm version patch
git push
```

### Workflow fails: "npm publish" error

**Common causes**:
1. **Version already exists on NPM**: Bump version and try again
2. **NPM token invalid**: Regenerate token and update GitHub Secret
3. **Package name taken**: Check NPM for naming conflicts
4. **Build failed**: Check build logs, fix errors, rebuild

### Local dependency errors

**Cause**: Package has `"file:../"` dependency that wasn't fixed.

**Solution**: The workflow automatically fixes these, but you can permanently fix:
```bash
# Edit package.json dependencies
"@chartml/core": "^1.4.0"  # Instead of "file:../core"
```

### Missing .d.ts files error

**Cause**: Package exports TypeScript definitions but doesn't generate them.

**Solution**: We've removed the `"types"` field from exports. If you see this error, verify the package.json doesn't have:
```json
"exports": {
  ".": {
    "types": "./dist/index.d.ts"  // ← Should be removed
  }
}
```

## Manual Publishing (Not Recommended)

If you need to publish manually without the workflow:

```bash
cd /home/jason/repos/chartml

# Build all packages
npm run build

# Publish each package in order
cd packages/core && npm publish --access public
cd ../chart-pie && npm publish --access public
cd ../chart-scatter && npm publish --access public
cd ../chart-metric && npm publish --access public
cd ../markdown-common && npm publish --access public
cd ../react && npm publish --access public
cd ../markdown-it && npm publish --access public
cd ../markdown-react && npm publish --access public

# Create git tags manually
git tag @chartml/core@1.4.1
git push origin --tags
```

**Note**: This bypasses dependency fixes and doesn't create releases. Use the workflow instead.

## Rollback

If a publish goes wrong (within 72 hours):

```bash
# Unpublish from NPM
npm unpublish @chartml/package@version

# Delete git tag
git tag -d @chartml/package@version
git push origin :refs/tags/@chartml/package@version

# Delete GitHub release (go to releases page)

# Fix issue, bump version, re-publish
```

## Package Access

All packages are published with `--access public` since they're scoped (`@chartml/`).

## Questions?

See:
- [GitHub Actions Workflow](/.github/workflows/publish-npm.yml)
- [Package Detection Script](/scripts/detect-changed-packages.js)
- [Publishing Script](/scripts/publish-packages.js)
