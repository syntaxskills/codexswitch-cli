#!/usr/bin/env bash
set -euo pipefail

run_audit=1
run_tests=1
run_install_check=1
run_fetch=1
run_fmt=1
run_clippy=1

usage() {
  cat <<'EOF'
Usage: scripts/check.sh [--no-audit] [--no-tests] [--lint-only] [--tests-only] [--audit-only]
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --no-audit)
      run_audit=0
      shift
      ;;
    --no-tests)
      run_tests=0
      shift
      ;;
    --lint-only)
      run_tests=0
      run_audit=0
      shift
      ;;
    --tests-only)
      run_install_check=0
      run_fmt=0
      run_clippy=0
      run_audit=0
      shift
      ;;
    --audit-only)
      run_install_check=0
      run_fmt=0
      run_clippy=0
      run_tests=0
      run_audit=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ "${run_install_check}" -eq 1 ]]; then
  bash install.sh --help >/dev/null
fi
if [[ "${run_fetch}" -eq 1 ]]; then
  cargo fetch --locked
fi
if [[ "${run_fmt}" -eq 1 ]]; then
  cargo fmt --all -- --check
fi
if [[ "${run_clippy}" -eq 1 ]]; then
  cargo clippy --all-targets --locked -- -D warnings
fi
if [[ "${run_tests}" -eq 1 ]]; then
  if command -v cargo-nextest >/dev/null 2>&1; then
    cargo nextest run --tests --locked
  else
    cargo test --tests --locked
  fi
fi
if [[ "${run_audit}" -eq 1 ]]; then
  cargo audit
fi
