#!/usr/bin/env bash
set -euo pipefail

repository="${GITHUB_REPOSITORY:-syntaxskills/codexswitch-cli}"
mode="generated"
version="${1:-}"
out_dir="${2:-dist}"

usage() {
  cat <<'EOF'
Usage:
  scripts/verify-artifacts.sh [version] [out_dir]
  scripts/verify-artifacts.sh --release <version> [download_dir]
  scripts/verify-artifacts.sh --release-dir <version> <download_dir>

Without an option, verifies locally generated release output (default: dist).
--release downloads every asset from the tagged GitHub release before verifying
it. --release-dir verifies an already downloaded, complete set of release assets.
EOF
}

release_recovery_help() {
  local tag="$1"
  local release_dir="$2"

  cat >&2 <<EOF
Cannot verify ${tag}: the release must contain both SHA256SUMS and
release-manifest.json together with every artifact named by SHA256SUMS.

First confirm GitHub connectivity and authentication with 'gh auth status'.
Do not invent or reconstruct checksum values without the original release
artifacts. Maintainers can recover the immutable tag through the release
workflow, after confirming registry publishing credentials are configured:

  gh workflow run release.yml --repo ${repository} -f tag=${tag}

Then download and verify the published assets:

  scripts/verify-artifacts.sh --release ${tag} ${release_dir}
EOF
}

verify_checksum_set() {
  python3 - "$@" <<'PY'
import hashlib
import json
import pathlib
import re
import sys

version, checksums_path, manifest_path, label, *artifact_roots = sys.argv[1:]
artifact_roots = [pathlib.Path(path).resolve() for path in artifact_roots]

expected = {}
with open(checksums_path, "r", encoding="utf-8") as fh:
    for line_number, line in enumerate(fh, 1):
        line = line.rstrip("\n")
        if not line:
            continue
        try:
            sha256, artifact = line.split("  ", 1)
        except ValueError:
            raise SystemExit(
                f"Malformed checksum line {line_number}: expected '<sha256>  <file>'"
            )
        if not re.fullmatch(r"[0-9a-fA-F]{64}", sha256):
            raise SystemExit(f"Invalid SHA-256 on checksum line {line_number}")
        if pathlib.PurePath(artifact).name != artifact:
            raise SystemExit(
                f"Unsafe checksum path on line {line_number}: {artifact}"
            )
        if artifact in expected:
            raise SystemExit(f"Duplicate checksum entry: {artifact}")
        expected[artifact] = sha256.lower()

if not expected:
    raise SystemExit("SHA256SUMS does not contain any artifacts")

with open(manifest_path, "r", encoding="utf-8") as fh:
    manifest = json.load(fh)

if manifest.get("version") != version:
    raise SystemExit(
        f"Manifest version mismatch: {manifest.get('version')} != {version}"
    )
if manifest.get("tag") != f"v{version}":
    raise SystemExit(f"Manifest tag mismatch: {manifest.get('tag')} != v{version}")

repository = manifest.get("repository")
if not isinstance(repository, dict) or not repository.get("slug") or not repository.get("url"):
    raise SystemExit("Manifest repository field must include slug and url")
if "commit" not in manifest:
    raise SystemExit("Manifest commit field is missing")

tools = manifest.get("tools")
if not isinstance(tools, dict) or not tools:
    raise SystemExit("Manifest tools field must be a non-empty object")

provenance = manifest.get("provenance")
if not isinstance(provenance, dict):
    raise SystemExit("Manifest provenance field must be an object")
for key in ("github_release", "verification_guide", "github_attestations", "npm_provenance"):
    if key not in provenance:
        raise SystemExit(f"Manifest provenance field is missing {key}")

artifacts = manifest.get("artifacts")
if not isinstance(artifacts, list):
    raise SystemExit("Manifest artifacts field must be a list")

observed = {}
for artifact in artifacts:
    if not isinstance(artifact, dict):
        raise SystemExit("Manifest artifact entries must be objects")
    path = artifact.get("path")
    sha256 = artifact.get("sha256")
    if not path or not sha256:
        raise SystemExit("Manifest artifact entries must include path and sha256")
    if path in observed:
        raise SystemExit(f"Duplicate manifest artifact entry: {path}")
    observed[path] = sha256.lower()
if observed != expected:
    raise SystemExit("Manifest artifacts do not match SHA256SUMS")

for artifact, expected_sha in expected.items():
    matches = [root / artifact for root in artifact_roots if (root / artifact).is_file()]
    if not matches:
        raise SystemExit(
            f"Missing {label} artifact named by SHA256SUMS: {artifact}"
        )
    if len(matches) > 1:
        raise SystemExit(
            f"Ambiguous checksummed artifact appears in multiple dirs: {artifact}"
        )

    digest = hashlib.sha256()
    with matches[0].open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            digest.update(chunk)
    actual_sha = digest.hexdigest()
    if actual_sha != expected_sha:
        raise SystemExit(
            f"Checksum mismatch for {artifact}: {actual_sha} != {expected_sha}"
        )

print(f"Verified {len(expected)} {label} artifacts for v{version}")
PY
}

if [[ "${version}" == "-h" || "${version}" == "--help" ]]; then
  usage
  exit 0
fi

if [[ "${version}" == "--release" || "${version}" == "--release-dir" ]]; then
  mode="${version#--}"
  version="${2:-}"
  out_dir="${3:-}"

  if [[ -z "${version}" ]]; then
    usage >&2
    exit 1
  fi

  version="${version#v}"
  tag="v${version}"
  if [[ -z "${out_dir}" ]]; then
    if [[ "${mode}" == "release-dir" ]]; then
      usage >&2
      exit 1
    fi
    out_dir="dist/releases/${tag}"
  fi

  if [[ "${mode}" == "release" ]]; then
    if ! command -v gh >/dev/null 2>&1; then
      echo "Missing GitHub CLI (gh), required to download ${tag}." >&2
      echo "Install gh and authenticate with 'gh auth login', then retry." >&2
      exit 1
    fi

    if ! asset_names="$(
      gh release view "${tag}" \
        --repo "${repository}" \
        --json assets \
        --jq '.assets[].name'
    )"; then
      echo "Unable to read GitHub Release ${tag} from ${repository}." >&2
      release_recovery_help "${tag}" "${out_dir}"
      exit 1
    fi

    if [[ -z "${asset_names}" ]] ||
       ! grep -Fxq "SHA256SUMS" <<<"${asset_names}" ||
       ! grep -Fxq "release-manifest.json" <<<"${asset_names}"; then
      echo "GitHub Release ${tag} does not publish the required checksum metadata." >&2
      release_recovery_help "${tag}" "${out_dir}"
      exit 1
    fi

    mkdir -p "${out_dir}"
    if ! gh release download "${tag}" \
      --repo "${repository}" \
      --dir "${out_dir}" \
      --clobber; then
      echo "Failed to download all assets for GitHub Release ${tag}." >&2
      release_recovery_help "${tag}" "${out_dir}"
      exit 1
    fi
  fi

  checksums_file="${out_dir}/SHA256SUMS"
  manifest_file="${out_dir}/release-manifest.json"

  if [[ ! -s "${checksums_file}" || ! -f "${manifest_file}" ]]; then
    echo "Incomplete release assets in ${out_dir}." >&2
    release_recovery_help "${tag}" "${out_dir}"
    exit 1
  fi

  verify_checksum_set \
    "${version}" \
    "${checksums_file}" \
    "${manifest_file}" \
    "release" \
    "${out_dir}"
  exit 0
fi

if [[ -z "${version}" ]]; then
  version=$(python3 - <<'PY'
import json
with open("package.json", "r", encoding="utf-8") as fh:
    print(json.load(fh)["version"])
PY
  )
fi

version="${version#v}"

release_dir="${out_dir}/release"
npm_packages_dir="${out_dir}/npm-packages"
homebrew_dir="${out_dir}/homebrew"
cargo_dir="${out_dir}/cargo"
checksums_file="${out_dir}/checksums/SHA256SUMS"
manifest_file="${out_dir}/checksums/release-manifest.json"

if [[ ! -d "${release_dir}" ]]; then
  echo "Missing release dir: ${release_dir}" >&2
  exit 1
fi

if [[ ! -d "${npm_packages_dir}" ]]; then
  echo "Missing npm packages dir: ${npm_packages_dir}" >&2
  exit 1
fi

if [[ ! -d "${cargo_dir}" ]]; then
  echo "Missing cargo dir: ${cargo_dir}" >&2
  exit 1
fi

if [[ ! -f "${checksums_file}" ]]; then
  echo "Missing checksums file: ${checksums_file}" >&2
  exit 1
fi

has_release_assets=0
shopt -s nullglob
for artifact_dir in "${out_dir}/artifacts"/codexswitch-cli-*; do
  target="${artifact_dir##*/codexswitch-cli-}"
  if [[ "${target}" == *windows* ]]; then
    expected="${release_dir}/codexswitch-cli-${target}.exe.zip"
  else
    expected="${release_dir}/codexswitch-cli-${target}.tar.gz"
  fi
  if [[ ! -f "${expected}" ]]; then
    echo "Missing release asset: ${expected}" >&2
    exit 1
  fi
  has_release_assets=1
done
shopt -u nullglob

if [[ "${has_release_assets}" -eq 0 ]]; then
  echo "No build artifacts found under ${out_dir}/artifacts" >&2
  exit 1
fi

main_pkg="${npm_packages_dir}/syntaxskills-codexswitch-cli-${version}.tgz"
if [[ ! -f "${main_pkg}" ]]; then
  echo "Missing npm main package: ${main_pkg}" >&2
  exit 1
fi

crate="${cargo_dir}/codexswitch-cli-${version}.crate"
if [[ ! -f "${crate}" ]]; then
  echo "Missing cargo crate: ${crate}" >&2
  exit 1
fi

if [[ -f "${release_dir}/codexswitch-cli-aarch64-apple-darwin.tar.gz" || \
      -f "${release_dir}/codexswitch-cli-x86_64-apple-darwin.tar.gz" ]]; then
  if [[ ! -f "${homebrew_dir}/codexswitch-cli.rb" ]]; then
    echo "Missing Homebrew cask: ${homebrew_dir}/codexswitch-cli.rb" >&2
    exit 1
  fi
fi

if [[ ! -s "${checksums_file}" ]]; then
  echo "Checksums file is empty: ${checksums_file}" >&2
  exit 1
fi

if [[ ! -f "${manifest_file}" ]]; then
  echo "Missing release manifest: ${manifest_file}" >&2
  exit 1
fi

verify_checksum_set \
  "${version}" \
  "${checksums_file}" \
  "${manifest_file}" \
  "generated" \
  "${release_dir}" \
  "${npm_packages_dir}" \
  "${cargo_dir}" \
  "${homebrew_dir}"
