# Maintainer Release Guide

Stable releases publish the same version to GitHub Releases, npm, and crates.io.
The release workflow is intentionally fail-closed: a missing publisher
configuration or failed registry upload fails the workflow.

Generated checksums are published as GitHub Release assets. The tag-triggered
workflow never commits or pushes generated files back to `main`.

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

## Recovery

The release workflow accepts an existing tag through `workflow_dispatch`.
Registry publication is idempotent: versions already present on crates.io are
detected and skipped, while unpublished versions must authenticate and publish
successfully before the workflow can complete.

Never move a published tag.
