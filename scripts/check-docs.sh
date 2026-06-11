#!/usr/bin/env bash
set -euo pipefail

readme_lines=$(wc -l < README.md | tr -d ' ')
if [[ "${readme_lines}" -ge 200 ]]; then
  echo "README.md must stay under 200 lines; current: ${readme_lines}" >&2
  exit 1
fi

required_links=(
  "docs/usage.md"
  "docs/verification.md"
  "CONTRIBUTING.md"
)

for path in "${required_links[@]}"; do
  if [[ ! -f "${path}" ]]; then
    echo "Missing README-linked file: ${path}" >&2
    exit 1
  fi
done

for path in README.md docs/*.md CONTRIBUTING.md CHANGELOG.md; do
  [[ -f "${path}" ]] || continue
  if grep -n '[[:blank:]]$' "${path}"; then
    echo "Trailing whitespace found in ${path}" >&2
    exit 1
  fi
done
