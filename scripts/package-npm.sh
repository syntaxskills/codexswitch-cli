#!/usr/bin/env bash
set -euo pipefail

version="${1:-}"
artifacts_dir="${2:-dist/artifacts}"
out_dir="${3:-dist/npm}"

if [[ -z "${version}" ]]; then
  echo "Usage: $0 <version> [artifacts_dir] [out_dir]" >&2
  exit 1
fi

version="${version#v}"

if [[ ! -d "${artifacts_dir}" ]]; then
  echo "Missing artifacts dir: ${artifacts_dir}" >&2
  exit 1
fi

rm -rf "${out_dir}"
mkdir -p "${out_dir}"

shopt -s nullglob
artifact_dirs=("${artifacts_dir}"/codexswitch-cli-*)
if [[ ${#artifact_dirs[@]} -eq 0 ]]; then
  echo "No build artifacts found under ${artifacts_dir}" >&2
  exit 1
fi

for artifact_dir in "${artifact_dirs[@]}"; do
  target="${artifact_dir##*/codexswitch-cli-}"
  pkg=""
  os=""
  cpu=""
  bin_name="codexswitch-cli"

  case "${target}" in
    x86_64-unknown-linux-gnu)
      pkg="@syntaxskills/codexswitch-cli-linux-x64"
      os="linux"
      cpu="x64"
      ;;
    aarch64-unknown-linux-gnu)
      pkg="@syntaxskills/codexswitch-cli-linux-arm64"
      os="linux"
      cpu="arm64"
      ;;
    x86_64-apple-darwin)
      pkg="@syntaxskills/codexswitch-cli-darwin-x64"
      os="darwin"
      cpu="x64"
      ;;
    aarch64-apple-darwin)
      pkg="@syntaxskills/codexswitch-cli-darwin-arm64"
      os="darwin"
      cpu="arm64"
      ;;
    x86_64-pc-windows-msvc)
      pkg="@syntaxskills/codexswitch-cli-win32-x64"
      os="win32"
      cpu="x64"
      bin_name="codexswitch-cli.exe"
      ;;
    *)
      echo "Skipping unsupported target ${target}" >&2
      continue
      ;;
  esac

  pkg_dir="${out_dir}/${pkg}"
  mkdir -p "${pkg_dir}/bin"
  if [[ ! -f "${artifact_dir}/${bin_name}" ]]; then
    echo "Missing binary for ${target}: ${artifact_dir}/${bin_name}" >&2
    exit 1
  fi
  cp "${artifact_dir}/${bin_name}" "${pkg_dir}/bin/${bin_name}"
  if [[ "${bin_name}" != *".exe" ]]; then
    chmod +x "${pkg_dir}/bin/${bin_name}"
  fi

  cat > "${pkg_dir}/package.json" <<JSON
{
  "name": "${pkg}",
  "version": "${version}",
  "license": "MIT",
  "os": ["${os}"],
  "cpu": ["${cpu}"],
  "files": ["bin"],
  "description": "Prebuilt native binary for CodexSwitch CLI",
  "repository": {
    "type": "git",
    "url": "git+https://github.com/syntaxskills/codexswitch-cli.git"
  }
}
JSON
done
shopt -u nullglob
