# Team Configuration

- **Team key:** CHA
- **Team name:** ChartML
- **GitHub repo:** kyomi-ai/chartml

## Versioning

- **Method:** semver
- **Tag format:** vMAJOR.MINOR.PATCH
- **Version source:** cargo-toml (workspace version in Cargo.toml is source of truth, must match tag)

## Release

- **Pipelines:** publish-crates.yml, publish-npm.yml
- **Production URL:** (library — no deployment)
- **Post-release:** crates.io + npm publish triggered by tag push
