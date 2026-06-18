# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- made npm publication mandatory for stable production releases and added
  post-publish verification for all npm packages, crates.io, and GitHub Release
- made `rust-toolchain.toml` the workflow toolchain source and added an
  executable check that keeps it aligned with the package MSRV
- refreshed compatible Rust dependencies, migrated `shlex` to 2.x, and added
  Dependabot coverage for Cargo, npm, and GitHub Actions; refreshed the Rust
  cache action to its Node 24 release
- unified every `--json` response under schema version 1 with consistent
  success and error envelopes
- split profile JSON rendering and CLI JSON contract tests into focused
  modules without changing public interfaces

## [2.0.0] - 2026-06-18

### Added

- published Rust library documentation under the `codexswitch_cli` crate name

### Changed

- isolated CodexSwitch profile storage under `~/.codex/codexswitch/profiles/`
- made stable crates.io publication fail closed and use short-lived trusted
  publishing credentials after the one-time initial package bootstrap
- stopped tag-triggered releases from committing generated checksums to `main`;
  checksums remain attached to each GitHub Release
- removed environment variable aliases inherited from the predecessor project
  from the CLI, npm wrapper, and installer

### Removed

- removed obsolete repository checksum copies for pre-CodexSwitch artifacts;
  historical checksums remain attached to their GitHub Releases

## [0.3.2] - 2026-06-11

### Fixed

- cleared `rustls-webpki` audit advisories by refreshing locked dependencies

## [0.3.1] - 2026-06-11

### Added

- `--include-config` support for saving and restoring profile-specific `config.toml` sidecars

### Changed

- prepared the standalone CodexSwitch CLI release workflow and package metadata

## [0.3.0] - 2026-06-11

### Added

- `doctor` command with `--fix` and `--json` for scriptable diagnostics and safe storage repairs
- profile label management commands: `label set`, `label clear`, and `label rename`
- profile export and import commands for backup and transfer bundles
- exact `--id` selectors for `load`, `delete`, and `status`, plus saved-profile `status` selectors with `--label` and `--id`
- `load --force`, `list --json`, `list --show-id`, and machine-readable `--json` output for mutating commands, `status`, and `doctor`

### Changed

- `--json` is now a global CLI option, and mutating commands emit a consistent success response shape
- `status` now refreshes usage auth on `401` responses, syncs refreshed credentials back to the saved active profile when possible, and formats auth/usage HTTP errors in a Codex-style layout
- `status --all --json` now returns a single `profiles` array that includes API-key and errored profiles, and human-readable output now uses "active profile" wording
- top-level `--help` output now uses shorter examples, highlights `--json` support more clearly, and label subcommands now spell out the required `--label`/`--id` selector more clearly
- the manual installer now resolves the latest published GitHub release automatically when no version is pinned, while still honoring `CODEXSWITCH_CLI_VERSION` and `--version`
- Cargo installs now require Rust 1.94 or newer
- package and README metadata now describe the tool in terms of switching between multiple Codex accounts
- status usage lookups now only allow official ChatGPT hosts or loopback addresses for `chatgpt_base_url`, and non-file Codex auth store modes are rejected

### Fixed

- usage fetching now retries transient transport, `5xx`, and rate-limit failures more robustly and supports grouped multi-bucket usage limits
- installer and release downloads now verify checksums from the tagged GitHub release by default, with a guarded insecure bypass for environments that explicitly set `CODEXSWITCH_CLI_ALLOW_INSECURE_INSTALL=1`
- npm and Bun update-source detection is more reliable
- regular remote error output now uses aligned multiline blocks while JSON output preserves raw backend detail
- errors always exit non-zero and write to stderr only; stdout is never polluted with partial JSON on error

### Removed

- `status --all --show-errors` flag and hidden-profile summary/count modes

### Internal

- `anyhow` dependency was removed; `updates.rs` now uses `Result<T, String>` consistently
- auth refresh now refuses to rewrite drifted on-disk auth state
- release packaging now emits a release manifest, verifies generated artifacts more strictly, and publishes npm packages with provenance plus GitHub attestations
- release and CI workflows were hardened, pinned more explicitly, and now include a scheduled security-audit workflow
- integration and regression coverage expanded for JSON output, release artifacts, status/profile edge cases, and profile storage repair paths

### Documentation

- README now documents export/import, label workflows, doctor repair behavior, and release verification more thoroughly
- added a release verification guide covering `SHA256SUMS`, `release-manifest.json`, GitHub attestations, npm provenance, and `.crate` verification

## [0.2.0] - 2026-02-21

### Added

- `status --all --show-errors` to include errored profiles in the output
- Hidden-profile summaries in `status --all` (API profiles and errored profiles)

### Fixed

- Profile deduplication now uses a composite identity key (`principal_id` + `workspace_or_org_id` + `plan_type`) to prevent false matches
- Profile lifecycle operations now preserve update cache and handle save/load/delete edge cases more safely
- Installer trap variable scoping bug that could break cleanup on some shells
- Release bump flow now syncs npm optional package versions and installer default `VERSION` correctly
- CI cache config updated to remove deprecated `rust-cache` parameters and invalid `save-if` usage

### Changed

- Simplified profile state machine and status rendering logic
- Profile ordering changed from last-used timestamp sorting to deterministic profile-id ordering
- `list` and `status` no longer perform implicit current-profile sync side effects
- "Unknown last-used" badge is now hidden instead of shown as a placeholder

### Removed

- Last-used timestamp tracking in `profiles.json` profile metadata
- Legacy `cx` shorthand script (use `codexswitch-cli` directly)

### Internal

- Installer and release scripts hardened for edge-case reliability
- CI: removed unused `sccache` from workflow jobs
- Added repository checksum file for previous release (`checksums/v0.1.0.txt`)

### Documentation

- README: reorganized badges, added a tests badge, and fixed license badge color
- README: added Cargo installation instructions
- CONTRIBUTING: documented release-tag bump behavior for npm optional packages and installer defaults

## [0.1.0] - 2026-01-28

### Added

**Core Features**
- Save and load Codex CLI authentication profiles with optional labels
- Interactive profile picker with search and navigation
- List all profiles ordered by last used timestamp
- Delete profiles with confirmation prompts
- Display usage statistics (requests, costs) for profiles
- Automatic OAuth token refresh when loading expired profiles
- Support for both OAuth tokens and API keys

**CLI Experience**
- Terminal styling with color support (respects `NO_COLOR` and `FORCE_COLOR`)
- `--plain` flag to disable all styling and use raw output
- Clear error messages with actionable suggestions
- Command examples in `--help` output

**Installation**
- Smart installer script with automatic OS/architecture detection
- Checksum verification for secure downloads
- Download progress indicators with TTY detection
- Cross-platform support (Linux, macOS, Windows)
- Multiple installation methods: npm, Bun, Cargo, manual script
- Automated update checking with 24-hour interval

**Technical**
- File locking for safe concurrent operations
- Profile storage in `~/.codex/profiles/`
- Atomic file writes to prevent corruption
- 150 tests covering core functionality
- Pre-commit hooks for code quality
- Binary releases for 5 platforms (Linux x64/ARM64, macOS Intel/Apple Silicon, Windows x64)

[Unreleased]: https://github.com/syntaxskills/codexswitch-cli/compare/v0.3.2...HEAD
[0.3.2]: https://github.com/syntaxskills/codexswitch-cli/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/syntaxskills/codexswitch-cli/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/syntaxskills/codexswitch-cli/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/syntaxskills/codexswitch-cli/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/syntaxskills/codexswitch-cli/releases/tag/v0.1.0
