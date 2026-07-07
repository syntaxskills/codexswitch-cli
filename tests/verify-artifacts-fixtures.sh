#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/codexswitch-verify-artifacts.XXXXXX")"
trap 'rm -rf "${tmp_dir}"' EXIT

sha256_text() {
  python3 - "$1" <<'PY'
import hashlib
import sys

print(hashlib.sha256(sys.argv[1].encode("utf-8")).hexdigest())
PY
}

write_manifest() {
  local dir="$1"
  local artifact="$2"
  local sha256="$3"

  cat > "${dir}/release-manifest.json" <<JSON
{
  "version": "1.2.3",
  "tag": "v1.2.3",
  "repository": {
    "slug": "syntaxskills/codexswitch-cli",
    "url": "https://github.com/syntaxskills/codexswitch-cli"
  },
  "commit": "abc123",
  "generated_at": "2026-01-01T00:00:00Z",
  "tools": {
    "node": "v20.0.0"
  },
  "provenance": {
    "github_release": "https://github.com/syntaxskills/codexswitch-cli/releases/tag/v1.2.3",
    "verification_guide": "https://github.com/syntaxskills/codexswitch-cli/blob/v1.2.3/docs/verification.md",
    "github_attestations": true,
    "npm_provenance": true
  },
  "artifacts": [
    {
      "path": "${artifact}",
      "sha256": "${sha256}",
      "category": "release"
    }
  ]
}
JSON
}

expect_failure() {
  local name="$1"
  local expected="$2"
  shift 2

  local log="${tmp_dir}/${name}.log"
  if "$@" >"${log}" 2>&1; then
    echo "Expected failure for ${name}" >&2
    exit 1
  fi
  if ! grep -Fq "${expected}" "${log}"; then
    echo "Expected ${name} output to contain: ${expected}" >&2
    echo "--- output ---" >&2
    cat "${log}" >&2
    echo "--------------" >&2
    exit 1
  fi
}

make_case_dir() {
  local name="$1"
  local dir="${tmp_dir}/${name}"
  mkdir -p "${dir}"
  printf '%s' "actual artifact content" > "${dir}/artifact.txt"
  printf '%s\n' "${dir}"
}

valid_dir="$(make_case_dir valid)"
valid_sha="$(sha256_text "actual artifact content")"
printf '%s  artifact.txt\n' "${valid_sha}" > "${valid_dir}/SHA256SUMS"
write_manifest "${valid_dir}" "artifact.txt" "${valid_sha}"
bash "${repo_root}/scripts/verify-artifacts.sh" --release-dir v1.2.3 "${valid_dir}" >/dev/null

mismatch_dir="$(make_case_dir checksum-mismatch)"
wrong_sha="$(sha256_text "different content")"
printf '%s  artifact.txt\n' "${wrong_sha}" > "${mismatch_dir}/SHA256SUMS"
write_manifest "${mismatch_dir}" "artifact.txt" "${wrong_sha}"
expect_failure \
  "checksum-mismatch" \
  "Checksum mismatch for artifact.txt" \
  bash "${repo_root}/scripts/verify-artifacts.sh" --release-dir v1.2.3 "${mismatch_dir}"

unsafe_checksum_dir="$(make_case_dir unsafe-checksum)"
printf '%064d  ../artifact.txt\n' 0 > "${unsafe_checksum_dir}/SHA256SUMS"
write_manifest "${unsafe_checksum_dir}" "../artifact.txt" "0000000000000000000000000000000000000000000000000000000000000000"
expect_failure \
  "unsafe-checksum" \
  "Unsafe checksum path on line 1: ../artifact.txt" \
  bash "${repo_root}/scripts/verify-artifacts.sh" --release-dir v1.2.3 "${unsafe_checksum_dir}"

unsafe_manifest_dir="$(make_case_dir unsafe-manifest)"
printf '%s  artifact.txt\n' "${valid_sha}" > "${unsafe_manifest_dir}/SHA256SUMS"
write_manifest "${unsafe_manifest_dir}" "../artifact.txt" "${valid_sha}"
expect_failure \
  "unsafe-manifest" \
  "Unsafe manifest artifact path: ../artifact.txt" \
  bash "${repo_root}/scripts/verify-artifacts.sh" --release-dir v1.2.3 "${unsafe_manifest_dir}"

missing_manifest_field_dir="$(make_case_dir missing-manifest-field)"
printf '%s  artifact.txt\n' "${valid_sha}" > "${missing_manifest_field_dir}/SHA256SUMS"
cat > "${missing_manifest_field_dir}/release-manifest.json" <<JSON
{
  "version": "1.2.3",
  "tag": "v1.2.3",
  "repository": {
    "slug": "syntaxskills/codexswitch-cli",
    "url": "https://github.com/syntaxskills/codexswitch-cli"
  },
  "commit": "abc123",
  "provenance": {
    "github_release": "https://github.com/syntaxskills/codexswitch-cli/releases/tag/v1.2.3",
    "verification_guide": "https://github.com/syntaxskills/codexswitch-cli/blob/v1.2.3/docs/verification.md",
    "github_attestations": true,
    "npm_provenance": true
  },
  "artifacts": [
    {
      "path": "artifact.txt",
      "sha256": "${valid_sha}",
      "category": "release"
    }
  ]
}
JSON
expect_failure \
  "missing-manifest-field" \
  "Manifest tools field must be a non-empty object" \
  bash "${repo_root}/scripts/verify-artifacts.sh" --release-dir v1.2.3 "${missing_manifest_field_dir}"

malformed_manifest_field_dir="$(make_case_dir malformed-manifest-field)"
printf '%s  artifact.txt\n' "${valid_sha}" > "${malformed_manifest_field_dir}/SHA256SUMS"
cat > "${malformed_manifest_field_dir}/release-manifest.json" <<JSON
{
  "version": "1.2.3",
  "tag": "v1.2.3",
  "repository": {
    "slug": "syntaxskills/codexswitch-cli",
    "url": "https://github.com/syntaxskills/codexswitch-cli"
  },
  "commit": "abc123",
  "tools": {
    "node": "v20.0.0"
  },
  "provenance": {
    "github_release": "https://github.com/syntaxskills/codexswitch-cli/releases/tag/v1.2.3",
    "verification_guide": "https://github.com/syntaxskills/codexswitch-cli/blob/v1.2.3/docs/verification.md",
    "github_attestations": true,
    "npm_provenance": true
  },
  "artifacts": "artifact.txt"
}
JSON
expect_failure \
  "malformed-manifest-field" \
  "Manifest artifacts field must be a list" \
  bash "${repo_root}/scripts/verify-artifacts.sh" --release-dir v1.2.3 "${malformed_manifest_field_dir}"

echo "verify-artifacts fixture tests passed"
