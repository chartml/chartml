#!/usr/bin/env node

/**
 * Detect which packages need publishing by comparing package.json versions
 * with existing git tags.
 *
 * Outputs JSON array of packages that need publishing:
 * [
 *   { "name": "@chartml/core", "version": "1.4.0", "path": "packages/core" },
 *   ...
 * ]
 */

import { readFileSync, readdirSync, statSync } from 'fs';
import { join, dirname } from 'path';
import { execSync } from 'child_process';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const rootDir = join(__dirname, '..');

/**
 * Get all git tags from the repository
 */
function getGitTags() {
  try {
    const output = execSync('git tag', { cwd: rootDir, encoding: 'utf-8' });
    return new Set(output.trim().split('\n').filter(Boolean));
  } catch (error) {
    console.error('Error fetching git tags:', error.message);
    return new Set();
  }
}

/**
 * Read package.json from a directory
 */
function readPackageJson(packagePath) {
  try {
    const content = readFileSync(join(packagePath, 'package.json'), 'utf-8');
    return JSON.parse(content);
  } catch (error) {
    console.error(`Error reading package.json at ${packagePath}:`, error.message);
    return null;
  }
}

/**
 * Find all packages in the packages/ directory
 */
function findPackages() {
  const packagesDir = join(rootDir, 'packages');
  const packages = [];

  try {
    const entries = readdirSync(packagesDir);

    for (const entry of entries) {
      const packagePath = join(packagesDir, entry);

      if (!statSync(packagePath).isDirectory()) {
        continue;
      }

      const pkg = readPackageJson(packagePath);
      if (!pkg || !pkg.name || !pkg.version) {
        console.error(`Skipping ${entry}: missing name or version in package.json`);
        continue;
      }

      // Skip private packages
      if (pkg.private) {
        console.error(`Skipping ${pkg.name}: marked as private`);
        continue;
      }

      packages.push({
        name: pkg.name,
        version: pkg.version,
        path: packagePath.replace(rootDir + '/', ''),
        dependencies: pkg.dependencies || {},
        devDependencies: pkg.devDependencies || {}
      });
    }
  } catch (error) {
    console.error('Error reading packages directory:', error.message);
    process.exit(1);
  }

  return packages;
}

/**
 * Check if a package version has been published (has a git tag)
 */
function needsPublishing(packageName, version, tags) {
  const tag = `${packageName}@${version}`;
  return !tags.has(tag);
}

/**
 * Main function
 */
function main() {
  const tags = getGitTags();
  const packages = findPackages();
  const changedPackages = [];

  for (const pkg of packages) {
    if (needsPublishing(pkg.name, pkg.version, tags)) {
      changedPackages.push({
        name: pkg.name,
        version: pkg.version,
        path: pkg.path
      });
      console.error(`✓ ${pkg.name}@${pkg.version} needs publishing (no git tag found)`);
    } else {
      console.error(`- ${pkg.name}@${pkg.version} already published (tag exists)`);
    }
  }

  // Output JSON to stdout (for GitHub Actions to capture)
  console.log(JSON.stringify(changedPackages, null, 2));

  // Exit with error if no packages need publishing (for CI)
  if (changedPackages.length === 0) {
    console.error('\nNo packages need publishing.');
  } else {
    console.error(`\nFound ${changedPackages.length} package(s) to publish.`);
  }
}

main();
