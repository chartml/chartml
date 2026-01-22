#!/usr/bin/env node

/**
 * Publish packages to NPM in dependency order
 *
 * Features:
 * - Fixes local path dependencies before publishing
 * - Publishes in correct dependency order (dependencies first)
 * - Supports dry-run mode
 * - Validates packages before publishing
 * - Reports success/failure
 */

import { readFileSync, writeFileSync, readdirSync } from 'fs';
import { join, dirname } from 'path';
import { execSync } from 'child_process';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const rootDir = join(__dirname, '..');

const isDryRun = process.argv.includes('--dry-run');

/**
 * Read package.json from a directory
 */
function readPackageJson(packagePath) {
  const fullPath = join(rootDir, packagePath, 'package.json');
  const content = readFileSync(fullPath, 'utf-8');
  return JSON.parse(content);
}

/**
 * Write package.json to a directory
 */
function writePackageJson(packagePath, pkg) {
  const fullPath = join(rootDir, packagePath, 'package.json');
  writeFileSync(fullPath, JSON.stringify(pkg, null, 2) + '\n');
}

/**
 * Find all packages in the monorepo
 */
function findAllPackages() {
  const packagesDir = join(rootDir, 'packages');
  const entries = readdirSync(packagesDir);
  const packages = {};

  for (const entry of entries) {
    const packagePath = `packages/${entry}`;
    const pkg = readPackageJson(packagePath);
    if (pkg.name && !pkg.private) {
      packages[pkg.name] = {
        name: pkg.name,
        version: pkg.version,
        path: packagePath,
        pkg: pkg
      };
    }
  }

  return packages;
}

/**
 * Fix local file: dependencies by replacing with NPM versions
 */
function fixLocalDependencies(packagePath, pkg, allPackages) {
  let modified = false;
  const backup = JSON.parse(JSON.stringify(pkg));

  // Check dependencies
  if (pkg.dependencies) {
    for (const [depName, depVersion] of Object.entries(pkg.dependencies)) {
      if (depVersion.startsWith('file:')) {
        // This is a local dependency - replace with NPM version
        if (allPackages[depName]) {
          const npmVersion = `^${allPackages[depName].version}`;
          console.error(`  Fixing ${depName}: ${depVersion} → ${npmVersion}`);
          pkg.dependencies[depName] = npmVersion;
          modified = true;
        } else {
          console.error(`  WARNING: Local dependency ${depName} not found in monorepo`);
        }
      }
    }
  }

  // Check devDependencies (less common, but possible)
  if (pkg.devDependencies) {
    for (const [depName, depVersion] of Object.entries(pkg.devDependencies)) {
      if (depVersion.startsWith('file:')) {
        if (allPackages[depName]) {
          const npmVersion = `^${allPackages[depName].version}`;
          console.error(`  Fixing ${depName}: ${depVersion} → ${npmVersion}`);
          pkg.devDependencies[depName] = npmVersion;
          modified = true;
        }
      }
    }
  }

  return { modified, backup };
}

/**
 * Build dependency graph and return packages in publish order
 */
function sortByDependencyOrder(packages, allPackages) {
  const sorted = [];
  const visited = new Set();
  const allPackageNames = new Set(Object.keys(allPackages));

  function visit(pkgName) {
    if (visited.has(pkgName)) {
      return;
    }

    const pkg = packages.find(p => p.name === pkgName);
    if (!pkg) {
      return; // Package not in the publish list
    }

    visited.add(pkgName);

    // Visit dependencies first
    const pkgData = allPackages[pkgName];
    if (pkgData && pkgData.pkg.dependencies) {
      for (const depName of Object.keys(pkgData.pkg.dependencies)) {
        // Only visit internal dependencies
        if (allPackageNames.has(depName)) {
          visit(depName);
        }
      }
    }

    sorted.push(pkg);
  }

  // Visit all packages in the publish list
  for (const pkg of packages) {
    visit(pkg.name);
  }

  return sorted;
}

/**
 * Publish a single package
 */
function publishPackage(packagePath, packageName, version, dryRun) {
  const fullPath = join(rootDir, packagePath);

  console.error(`\n📦 Publishing ${packageName}@${version}...`);

  try {
    if (dryRun) {
      console.error('  [DRY RUN] Would run: npm publish --access public');
      // In dry run, just validate the package
      execSync('npm pack --dry-run', {
        cwd: fullPath,
        stdio: 'inherit'
      });
    } else {
      execSync('npm publish --access public', {
        cwd: fullPath,
        stdio: 'inherit'
      });
      console.error(`  ✓ Successfully published ${packageName}@${version}`);
    }
    return true;
  } catch (error) {
    console.error(`  ✗ Failed to publish ${packageName}@${version}`);
    console.error(`  Error: ${error.message}`);
    return false;
  }
}

/**
 * Main function
 */
function main() {
  console.error(isDryRun ? '\n🔍 DRY RUN MODE - No actual publishing\n' : '\n📦 PUBLISHING TO NPM\n');

  // Read changed packages from stdin (piped from detect-changed-packages.js)
  // Or read from changed-packages.json file
  let changedPackages;
  try {
    changedPackages = JSON.parse(readFileSync('changed-packages.json', 'utf-8'));
  } catch (error) {
    console.error('Error: changed-packages.json not found. Run detect-changed-packages.js first.');
    process.exit(1);
  }

  if (changedPackages.length === 0) {
    console.error('No packages to publish.');
    return;
  }

  // Get all packages for dependency resolution
  const allPackages = findAllPackages();

  // Sort packages by dependency order
  const sortedPackages = sortByDependencyOrder(changedPackages, allPackages);

  console.error('Publish order:');
  sortedPackages.forEach((pkg, i) => {
    console.error(`  ${i + 1}. ${pkg.name}@${pkg.version}`);
  });

  const results = {
    success: [],
    failed: [],
    skipped: []
  };

  // Fix local dependencies and publish each package
  for (const pkg of sortedPackages) {
    const packageData = readPackageJson(pkg.path);
    const { modified, backup } = fixLocalDependencies(pkg.path, packageData, allPackages);

    if (modified) {
      console.error(`\n🔧 Fixed local dependencies in ${pkg.name}`);
      writePackageJson(pkg.path, packageData);
    }

    // Publish the package
    const success = publishPackage(pkg.path, pkg.name, pkg.version, isDryRun);

    if (success) {
      results.success.push(pkg);
    } else {
      results.failed.push(pkg);
    }

    // Restore original package.json if we modified it
    if (modified && !isDryRun) {
      console.error(`🔄 Restoring original package.json for ${pkg.name}`);
      writePackageJson(pkg.path, backup);
    }
  }

  // Print summary
  console.error('\n' + '='.repeat(60));
  console.error('PUBLISH SUMMARY');
  console.error('='.repeat(60));

  if (results.success.length > 0) {
    console.error(`\n✓ Successfully published (${results.success.length}):`);
    results.success.forEach(pkg => {
      console.error(`  - ${pkg.name}@${pkg.version}`);
    });
  }

  if (results.failed.length > 0) {
    console.error(`\n✗ Failed to publish (${results.failed.length}):`);
    results.failed.forEach(pkg => {
      console.error(`  - ${pkg.name}@${pkg.version}`);
    });
  }

  if (isDryRun) {
    console.error('\n🔍 This was a dry run - no packages were actually published');
  }

  // Exit with error if any packages failed
  if (results.failed.length > 0) {
    process.exit(1);
  }
}

// Run main
try {
  main();
} catch (error) {
  console.error('Fatal error:', error);
  process.exit(1);
}
