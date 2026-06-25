# Release Verification

A complete GitHub release includes:

- `SHA256SUMS`
- `release-manifest.json`
- GitHub artifact attestations for release assets
- npm provenance for published npm packages

## Verify GitHub release assets

The project verifier downloads the complete asset set and checks every file
listed in `SHA256SUMS` against both its actual content and
`release-manifest.json`:

```bash
scripts/verify-artifacts.sh --release vX.Y.Z
```

Replace `vX.Y.Z` with the release tag you want to verify.

The same verifier is reusable in three release stages:

```bash
# Locally generated release output, before publishing.
scripts/verify-artifacts.sh X.Y.Z dist

# Published release assets, downloaded by the verifier.
scripts/verify-artifacts.sh --release vX.Y.Z

# Published or staged assets that you have already downloaded.
scripts/verify-artifacts.sh --release-dir vX.Y.Z /path/to/release-assets
```

For `--release-dir` or a manual verification, download every release asset into
a clean directory. Downloading only one binary with the complete `SHA256SUMS`
file makes standard checksum tools report the other release assets as missing.

```bash
TAG="vX.Y.Z"
mkdir -p "dist/releases/$TAG"
gh release download "$TAG" \
  --repo syntaxskills/codexswitch-cli \
  --dir "dist/releases/$TAG"
cd "dist/releases/$TAG"
sha256sum -c SHA256SUMS
```

On macOS, use `shasum -a 256 -c SHA256SUMS`.

`release-manifest.json` records the release version, tag, commit SHA, tool
versions, and the same per-asset hashes from `SHA256SUMS`.

## v2.0.0 and missing release assets

Checksums generated for v2.0.0 belong on the `v2.0.0` GitHub Release. There is
intentionally no `checksums/v2.0.0.txt` repository file: repository checksum
copies are retained only for older releases.

Verify v2.0.0 with:

```bash
scripts/verify-artifacts.sh --release v2.0.0
```

If the release, `SHA256SUMS`, `release-manifest.json`, or any checksummed
artifact is absent, verification is not possible. Do not create checksum values
for missing artifacts. The verifier fails with the exact maintainer recovery
command; after the existing immutable tag is recovered by the release workflow,
run the same verification command again.

## Verify GitHub attestations

Use the GitHub CLI to verify a release asset attestation:

```bash
gh attestation verify codexswitch-cli-x86_64-unknown-linux-gnu.tar.gz \
  -R syntaxskills/codexswitch-cli
```

Replace the asset name with the file you downloaded from the release.

## npm packages

npm packages are published with trusted publishing and provenance.

The matching npm tarballs are also uploaded to the GitHub release, so you can:

- verify their hashes with `SHA256SUMS`
- inspect them in `release-manifest.json`
- verify the GitHub release attestations for the uploaded tarballs

## crates.io package

The `.crate` published for crates.io is also uploaded to the GitHub release.
You can verify it the same way:

- compare its hash against `SHA256SUMS`
- confirm it appears in `release-manifest.json`
- verify the GitHub release attestation for the `.crate` asset
