# Contributing to CodexSwitch CLI

Thanks for helping improve CodexSwitch CLI. This project is a small Rust CLI for
saving, loading, listing, importing, exporting, and inspecting local Codex auth
profiles.

## What Fits

We welcome focused changes such as:

- Bug fixes with tests.
- Documentation and install-flow improvements.
- Small CLI usability improvements.
- Release, packaging, and CI fixes.
- Safety improvements around profile storage and auth files.

Please open an issue or discussion first for new commands, storage format
changes, auth/token handling changes, release workflow changes, or broad
refactors.

## Local Setup

- Use the Rust toolchain from `rust-toolchain.toml`.
- Use Node.js 20 or newer only if you touch npm packaging.
- Run `make hooks` if you want the repo-managed Git hooks.

Before sending a PR, run:

```bash
make precommit
```

If you do not have every optional local tool installed, run the core checks:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --tests --locked
```

## Pull Requests

- Keep each PR about one problem.
- Explain what changed, why it changed, and how you tested it.
- Add or update tests for behavior changes.
- Update README or docs when commands, output, install behavior, or release
  behavior changes.
- Include terminal output or screenshots when the CLI experience changes.

## Code Guidelines

- Keep auth and token handling conservative.
- Preserve compatibility with existing profiles and export bundles unless a
  migration is discussed first.
- Follow the existing Rust module style before adding new abstractions.
- Avoid new runtime dependencies unless they clearly reduce risk or complexity.
- Prefer actionable errors over silent fallback behavior.

## Releases

Releases are built from `v*` tags by GitHub Actions. Maintainers handle
publishing. The maintainer setup and recovery procedure are documented in
[`docs/releasing.md`](docs/releasing.md). Do not move a published tag.
