# Release Verification

Each GitHub release includes:

- `SHA256SUMS`
- `release-manifest.json`
- GitHub artifact attestations for release assets
- npm provenance for published npm packages

## Verify GitHub release assets

Download the release asset you want to inspect together with `SHA256SUMS` and
`release-manifest.json`:

```bash
TAG="vX.Y.Z"
gh release download "$TAG" \
  --repo syntaxskills/codexswitch-cli \
  --pattern 'SHA256SUMS' \
  --pattern 'release-manifest.json' \
  --pattern 'codexswitch-cli-x86_64-unknown-linux-gnu.tar.gz'
```

Replace `vX.Y.Z` with the release tag you want to verify.

Then verify the checksums:

```bash
shasum -a 256 -c SHA256SUMS
```

On systems with GNU coreutils:

```bash
sha256sum -c SHA256SUMS
```

`release-manifest.json` records the release version, tag, commit SHA, tool
versions, and the same per-asset hashes from `SHA256SUMS`.

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
