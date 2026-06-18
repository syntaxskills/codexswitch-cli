#!/usr/bin/env bash

set -euo pipefail

toolchain_version="$(
  sed -nE 's/^channel = "([^"]+)"$/\1/p' rust-toolchain.toml
)"
msrv="$(
  sed -nE 's/^rust-version = "([^"]+)"$/\1/p' Cargo.toml
)"
rustc_version="$(rustc --version | awk '{print $2}')"

if [[ -z "${toolchain_version}" || -z "${msrv}" ]]; then
  echo "Unable to read Rust versions from rust-toolchain.toml and Cargo.toml" >&2
  exit 1
fi

if [[ "${rustc_version}" != "${toolchain_version}" ]]; then
  echo "rustc ${rustc_version} does not match rust-toolchain.toml ${toolchain_version}" >&2
  exit 1
fi

if [[ "${toolchain_version%.*}" != "${msrv}" ]]; then
  echo "Cargo.toml rust-version ${msrv} does not match toolchain ${toolchain_version%.*}" >&2
  exit 1
fi

echo "Rust toolchain ${toolchain_version} matches Cargo MSRV ${msrv}"
