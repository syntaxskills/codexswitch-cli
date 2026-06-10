#!/usr/bin/env bash
set -euo pipefail

version="${1:-}"
out_dir="${2:-dist}"

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

main_pkg="${npm_packages_dir}/codexswitch-cli-${version}.tgz"
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

python3 - <<'PY' "${version}" "${checksums_file}" "${manifest_file}"
import json
import sys

version, checksums_path, manifest_path = sys.argv[1:]

expected = {}
with open(checksums_path, "r", encoding="utf-8") as fh:
    for line in fh:
        line = line.strip()
        if not line:
            continue
        sha256, path = line.split("  ", 1)
        expected[path] = sha256

with open(manifest_path, "r", encoding="utf-8") as fh:
    manifest = json.load(fh)

if manifest.get("version") != version:
    raise SystemExit(
        f"Manifest version mismatch: {manifest.get('version')} != {version}"
    )

if manifest.get("tag") != f"v{version}":
    raise SystemExit(
        f"Manifest tag mismatch: {manifest.get('tag')} != v{version}"
    )

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
    path = artifact.get("path")
    sha256 = artifact.get("sha256")
    if not path or not sha256:
        raise SystemExit("Manifest artifact entries must include path and sha256")
    observed[path] = sha256

if observed != expected:
    raise SystemExit("Manifest artifacts do not match SHA256SUMS")
PY
