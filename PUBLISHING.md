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

### NPM Trusted Publishing Setup (One-Time)

ChartML uses **NPM Trusted Publishing** (OIDC authentication), which is more secure than using long-lived tokens. No GitHub Secrets are required!

**Setup steps:**

1. **Configure each package on NPM** (do this once per package):
   - Log in to [npmjs.com](https://www.npmjs.com)
   - For each package (e.g., `@chartml/core`):
     - Go to the package settings
     - Navigate to "Publishing access" or "Automation tokens"
     - Configure GitHub Actions as a trusted publisher:
       - **Repository**: `chartml/chartml`
       - **Workflow**: `publish-npm.yml`
       - **Environment**: Leave blank (we don't use environments)

2. **That's it!** The workflow will automatically authenticate using OIDC tokens from GitHub Actions.

**Benefits of Trusted Publishing:**
- ✅ No long-lived tokens to manage or rotate
- ✅ No secrets stored in GitHub
- ✅ Packages include provenance attestations (verifiable build info)
- ✅ More secure - NPM verifies the publish came from your GitHub repo

**Note**: If this is the first time publishing a package, you may need to use a legacy token for the initial publish only. After that, configure Trusted Publishing for all future releases.

## Publishing Process

### Step 1: Bump Package Versions

Decide which packages need publishing and bump their versions:

```bash
# Bump a single package
cd packages/core
npm version patch  # 5.2.1 → 5.2.2 (or 'minor' or 'major')

# Or bump multiple packages (from the repository root)
npm version patch --workspaces  # Bumps all workspace packages
```

Peer ranges are not set by any script (`scripts/publish-packages.js` only
replaces `file:` dependencies). When a package starts using something new from
`@chartml/core`, raise its `@chartml/core` peer range (and the matching
`devDependencies` entry in `@chartml/markdown-react`) by hand to the first core
version that has it, then run `npm install` so `package-lock.json` follows.

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
2. **@chartml/datafusion**, **@chartml/markdown-it**, **@chartml/react** (peer-depend on core)
3. **@chartml/markdown-react** (peer-depends on core and markdown-it)

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
2. **Trusted Publishing not configured**: Configure the package on npmjs.com (see Prerequisites)
3. **Package name taken**: Check NPM for naming conflicts
4. **Build failed**: Check build logs, fix errors, rebuild
5. **First publish requires token**: For new packages, you may need to do the first publish manually with a token, then configure Trusted Publishing

### Local dependency errors

**Cause**: Package has `"file:../"` dependency that wasn't fixed.

**Solution**: The workflow automatically fixes these, but you can permanently fix:
```bash
# Edit package.json dependencies
"@chartml/core": "^5.0.0"  # Instead of "file:../core"
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
# From the repository root
npm run build

# Publish each npm package in order
cd packages/core && npm publish --access public
cd ../datafusion && npm publish --access public
cd ../markdown-it && npm publish --access public
cd ../react && npm publish --access public
cd ../markdown-react && npm publish --access public
cd ../..

# Publish the Rust crates to crates.io in dependency order
cargo publish -p chartml-core
cargo publish -p chartml-forecast
cargo publish -p chartml-chart-cartesian
cargo publish -p chartml-chart-metric
cargo publish -p chartml-chart-pie
cargo publish -p chartml-chart-scatter
cargo publish -p chartml-chart-table
cargo publish -p chartml-datafusion
cargo publish -p chartml-render
cargo publish -p chartml-leptos
# Wait about a minute between crates so the crates.io index catches up (CI sleeps 60s).

# Create git tags manually
git tag @chartml/core@5.2.2
git push origin --tags
```

**Note**: This bypasses dependency fixes and doesn't create releases. Use the workflow instead.

## Publishing the Rust crates

Pushing a `v*` tag runs `.github/workflows/publish-crates.yml`, which publishes
the crates in dependency order with `--no-verify`, so nothing in that workflow
compiles a crate against the other crates as they will be published. Check that
before tagging:

1. Set the versions. A change to `chartml-core`'s public API that breaks
   existing code (a new field on a struct built with a literal, a new variant of
   an enum matched exhaustively, a changed type) needs a new major version of
   `chartml-core`, and every crate that depends on it must be released with a
   requirement on that version (`chartml-core = { path = "...", version = "N" }`).
   Otherwise the chart crates already on crates.io, which ask for the old major
   version, are paired with a core they no longer compile against.
   `cargo install cargo-semver-checks`, then run
   `cargo semver-checks -p <crate>` for each published crate (it compares with
   the latest version on crates.io; add `--baseline-version X.Y.Z` to name one).
   It lists every breaking item. It does not report a field whose type changed,
   so read the CHANGELOG for those.
2. Commit the version bumps, then run `bash scripts/check-crates-package.sh`.
   It runs `cargo package` for every published crate in one run (the
   crates in `publish-crates.yml`, in its order): each crate is packaged as it
   will be published, with its path dependencies replaced by their version
   requirements, and built against the other crates of this repository, not
   against what crates.io holds. It fails when a crate no longer compiles
   against the new `chartml-core` or a version requirement does not match the
   new version. It needs cargo 1.90 or newer and a clean git tree.
3. Move the CHANGELOG's Unreleased entries under the new version.

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
