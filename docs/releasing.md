# Maintainer Release Guide

Stable releases publish the same version to GitHub Releases, npm, and crates.io.
The release workflow is intentionally fail-closed: a missing publisher
configuration or failed registry upload fails the workflow.

Generated checksums are published as GitHub Release assets. The tag-triggered
workflow never commits or pushes generated files back to `main`.

## Version Consistency Check

After changing a release version and before creating or pushing its tag, run:

```sh
npm run check:release-version
```

This validates that `Cargo.toml`, the `codexswitch-cli` entry in `Cargo.lock`,
`package.json`, and all npm platform packages in `optionalDependencies` use the
same version.

To also verify the version intended for a release, pass either the version or
tag:

```sh
npm run check:release-version -- 2.0.0
npm run check:release-version -- v2.0.0
```

The command exits nonzero and identifies every mismatch. Maintainers should
resolve all reported differences before running `scripts/release-tag` or
`scripts/release-prep.sh`.

## crates.io Initial Publish

crates.io trusted publishing can only be configured after the crate exists.
For the first `codexswitch-cli` publication:

1. Create a crates.io API token with permission to publish the crate.
2. Add it temporarily as the GitHub Actions secret
   `CARGO_REGISTRY_TOKEN`.
3. Run the `release` workflow manually for the existing stable tag and enable
   `bootstrap_crates_io`.
4. Confirm the version is visible at
   `https://crates.io/crates/codexswitch-cli`.

The bootstrap path fails if the secret is absent. It is never selected by tag
pushes.

## crates.io Trusted Publisher

After the first publication, configure a trusted publisher in the crate's
crates.io settings with:

| Field | Value |
| --- | --- |
| GitHub owner | `syntaxskills` |
| Repository | `codexswitch-cli` |
| Workflow | `release.yml` |
| Environment | leave blank |

Then remove the `CARGO_REGISTRY_TOKEN` GitHub Actions secret. Future stable
releases obtain a short-lived token through crates.io's official
`crates-io-auth-action`. The token is revoked automatically when the job ends.

Maintainers can verify the configuration without publishing a new version by
running the `release` workflow for an existing stable tag with
`verify_crates_io_trusted_publisher` enabled.

## npm Trusted Publishers

Every stable production release publishes the main npm package and five
platform packages. Each package must trust the same GitHub Actions workflow:

| Field | Value |
| --- | --- |
| Repository | `syntaxskills/codexswitch-cli` |
| Workflow | `release.yml` |
| Environment | leave blank |
| Permission | `npm publish` |

The required packages are:

- `@syntaxskills/codexswitch-cli`
- `@syntaxskills/codexswitch-cli-darwin-arm64`
- `@syntaxskills/codexswitch-cli-darwin-x64`
- `@syntaxskills/codexswitch-cli-linux-arm64`
- `@syntaxskills/codexswitch-cli-linux-x64`
- `@syntaxskills/codexswitch-cli-win32-x64`

The release workflow publishes platform packages before the main package and
then verifies all six npm versions, crates.io, and the GitHub Release before
succeeding.

## npm Package Smoke Test

Run the npm package smoke test after generating release artifacts and before
publishing them:

```sh
scripts/release-artifacts.sh "$VERSION" dist/artifacts dist
make smoke-test-npm
```

The test installs the main package and the current platform package from
`dist/npm-packages` into a temporary project, then runs
`codexswitch-cli --version` and `codexswitch-cli --help`. It uses temporary npm
configuration, cache, and prefix directories and does not publish packages or
install anything globally. The release workflow runs this check automatically
before artifact attestation and publication.

## Recovery

The release workflow accepts an existing tag through `workflow_dispatch`.
Registry publication is idempotent: versions already present on crates.io are
detected and skipped, while unpublished versions must authenticate and publish
successfully before the workflow can complete.

For example, if the v2.0.0 release assets or checksum metadata are absent, first
confirm the npm and crates.io trusted publishers are configured, then recover
the existing tag:

```bash
gh workflow run release.yml \
  --repo syntaxskills/codexswitch-cli \
  -f tag=v2.0.0
```

Do not add a repository checksum file or invent hashes while the artifacts are
missing. Once the workflow has uploaded the assets, verify the complete release:

```bash
scripts/verify-artifacts.sh --release v2.0.0
```

Never move a published tag.
