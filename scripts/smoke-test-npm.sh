#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
packages_dir="${1:-${repo_root}/dist/npm-packages}"
if [[ "${packages_dir}" != /* ]]; then
  packages_dir="$(pwd)/${packages_dir}"
fi

if ! command -v node >/dev/null 2>&1; then
  echo "Missing required command: node" >&2
  exit 1
fi

if ! command -v npm >/dev/null 2>&1; then
  echo "Missing required command: npm" >&2
  exit 1
fi

version="$(cd "${repo_root}" && node -p "require('./package.json').version")"
platform_key="$(node -p 'process.platform + "-" + process.arch')"

case "${platform_key}" in
  linux-x64|linux-arm64|darwin-x64|darwin-arm64|win32-x64)
    platform_package="syntaxskills-codexswitch-cli-${platform_key}-${version}.tgz"
    ;;
  *)
    echo "Unsupported npm package smoke-test platform: ${platform_key}" >&2
    echo "Supported platforms: linux-x64, linux-arm64, darwin-x64, darwin-arm64, win32-x64" >&2
    exit 1
    ;;
esac

main_package="${packages_dir}/syntaxskills-codexswitch-cli-${version}.tgz"
platform_package="${packages_dir}/${platform_package}"
missing=0

if [[ ! -f "${main_package}" ]]; then
  echo "Missing npm main package tarball: ${main_package}" >&2
  missing=1
fi

if [[ ! -f "${platform_package}" ]]; then
  echo "Missing npm platform package tarball for ${platform_key}: ${platform_package}" >&2
  missing=1
fi

if [[ "${missing}" -ne 0 ]]; then
  cat >&2 <<EOF
Populate dist/artifacts with the platform build outputs, generate the release
artifacts, then rerun this smoke test:
  scripts/release-artifacts.sh "${version}" dist/artifacts dist
  scripts/smoke-test-npm.sh dist/npm-packages
EOF
  exit 1
fi

tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/codexswitch-npm-smoke.XXXXXX")"
trap 'rm -rf "${tmp_dir}"' EXIT

mkdir -p \
  "${tmp_dir}/project" \
  "${tmp_dir}/home" \
  "${tmp_dir}/npm-cache" \
  "${tmp_dir}/npm-prefix"
: > "${tmp_dir}/npmrc"

(
  cd "${tmp_dir}/project"
  HOME="${tmp_dir}/home" \
    npm_config_cache="${tmp_dir}/npm-cache" \
    npm_config_prefix="${tmp_dir}/npm-prefix" \
    npm_config_userconfig="${tmp_dir}/npmrc" \
    npm install \
      --no-save \
      --no-package-lock \
      --no-audit \
      --no-fund \
      --ignore-scripts \
      --omit=optional \
      --offline \
      "${platform_package}" \
      "${main_package}"

  cli="node_modules/.bin/codexswitch-cli"
  if [[ ! -x "${cli}" ]]; then
    echo "Installed CLI launcher is missing or not executable: ${tmp_dir}/project/${cli}" >&2
    exit 1
  fi

  HOME="${tmp_dir}/home" "${cli}" --version
  HOME="${tmp_dir}/home" "${cli}" --help >/dev/null
)

echo "npm package smoke test passed for ${platform_key} (${version})"
