#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

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
  linux-x64)
    platform_package="@syntaxskills/codexswitch-cli-linux-x64"
    ;;
  linux-arm64)
    platform_package="@syntaxskills/codexswitch-cli-linux-arm64"
    ;;
  darwin-x64)
    platform_package="@syntaxskills/codexswitch-cli-darwin-x64"
    ;;
  darwin-arm64)
    platform_package="@syntaxskills/codexswitch-cli-darwin-arm64"
    ;;
  win32-x64)
    echo "Skipping lightweight npm wrapper smoke test on Windows; the wrapper expects a native .exe payload." >&2
    exit 0
    ;;
  *)
    echo "Unsupported npm wrapper smoke-test platform: ${platform_key}" >&2
    exit 1
    ;;
esac

tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/codexswitch-npm-wrapper.XXXXXX")"
trap 'rm -rf "${tmp_dir}"' EXIT

mkdir -p \
  "${tmp_dir}/packages" \
  "${tmp_dir}/platform/bin" \
  "${tmp_dir}/project" \
  "${tmp_dir}/home" \
  "${tmp_dir}/npm-cache" \
  "${tmp_dir}/npm-prefix"
: > "${tmp_dir}/npmrc"

(
  cd "${repo_root}"
  HOME="${tmp_dir}/home" \
    npm_config_cache="${tmp_dir}/npm-cache" \
    npm_config_loglevel=error \
    npm_config_prefix="${tmp_dir}/npm-prefix" \
    npm_config_userconfig="${tmp_dir}/npmrc" \
    npm pack --pack-destination "${tmp_dir}/packages" >/dev/null
)

main_package="${tmp_dir}/packages/syntaxskills-codexswitch-cli-${version}.tgz"
if [[ ! -f "${main_package}" ]]; then
  echo "Missing npm wrapper tarball: ${main_package}" >&2
  exit 1
fi

if ! tar -tzf "${main_package}" | grep -Fxq "package/bin/codexswitch-cli.js"; then
  echo "npm wrapper tarball does not contain bin/codexswitch-cli.js" >&2
  exit 1
fi

cat > "${tmp_dir}/platform/package.json" <<JSON
{
  "name": "${platform_package}",
  "version": "${version}",
  "license": "MIT",
  "files": ["bin"]
}
JSON

cat > "${tmp_dir}/platform/bin/codexswitch-cli" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
printf 'fake codexswitch native\n'
printf 'args=%s\n' "$*"
printf 'managed_by_npm=%s\n' "${CODEXSWITCH_CLI_MANAGED_BY_NPM:-}"
printf 'command=%s\n' "${CODEXSWITCH_CLI_COMMAND:-}"
SH
chmod +x "${tmp_dir}/platform/bin/codexswitch-cli"

HOME="${tmp_dir}/home" \
  npm_config_cache="${tmp_dir}/npm-cache" \
  npm_config_loglevel=error \
  npm_config_prefix="${tmp_dir}/npm-prefix" \
  npm_config_userconfig="${tmp_dir}/npmrc" \
  npm pack "${tmp_dir}/platform" --pack-destination "${tmp_dir}/packages" >/dev/null
platform_package_file="${tmp_dir}/packages/syntaxskills-codexswitch-cli-${platform_key}-${version}.tgz"
if [[ ! -f "${platform_package_file}" ]]; then
  echo "Missing temporary platform tarball: ${platform_package_file}" >&2
  exit 1
fi

(
  cd "${tmp_dir}/project"
  HOME="${tmp_dir}/home" \
    npm_config_cache="${tmp_dir}/npm-cache" \
    npm_config_loglevel=error \
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
      "${platform_package_file}" \
      "${main_package}"

  cli="node_modules/.bin/codexswitch-cli"
  if [[ ! -x "${cli}" ]]; then
    echo "Installed CLI launcher is missing or not executable: ${tmp_dir}/project/${cli}" >&2
    exit 1
  fi

  output="$(HOME="${tmp_dir}/home" "${cli}" --version)"
  grep -Fxq "fake codexswitch native" <<<"${output}" \
    || { echo "Wrapper did not execute the platform binary" >&2; exit 1; }
  grep -Fxq "args=--version" <<<"${output}" \
    || { echo "Wrapper did not forward CLI arguments" >&2; exit 1; }
  grep -Fxq "managed_by_npm=1" <<<"${output}" \
    || { echo "Wrapper did not set CODEXSWITCH_CLI_MANAGED_BY_NPM" >&2; exit 1; }
  grep -Fxq "command=codexswitch-cli" <<<"${output}" \
    || { echo "Wrapper did not set CODEXSWITCH_CLI_COMMAND" >&2; exit 1; }
)

echo "npm wrapper smoke test passed for ${platform_key} (${version})"
