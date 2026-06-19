<div align="center">

# CodexSwitch CLI

**Save once. Switch Codex accounts in one command.**

Fast, local-first profile switching for Codex, ChatGPT accounts, and custom providers.

[![Tests](https://github.com/syntaxskills/codexswitch-cli/actions/workflows/tests.yml/badge.svg?branch=main)](https://github.com/syntaxskills/codexswitch-cli/actions/workflows/tests.yml)
[![Docs](https://github.com/syntaxskills/codexswitch-cli/actions/workflows/docs.yml/badge.svg?branch=main)](https://github.com/syntaxskills/codexswitch-cli/actions/workflows/docs.yml)
[![Security audit](https://github.com/syntaxskills/codexswitch-cli/actions/workflows/security-audit.yml/badge.svg)](https://github.com/syntaxskills/codexswitch-cli/actions/workflows/security-audit.yml)
[![Release](https://github.com/syntaxskills/codexswitch-cli/actions/workflows/release.yml/badge.svg)](https://github.com/syntaxskills/codexswitch-cli/actions/workflows/release.yml)

[Install](#install) · [Quick start](#quick-start) · [Commands](#commands) · [Documentation](#documentation)

</div>

## Install

### Ask Your AI Agent

Copy this prompt into Codex, Claude Code, Cursor, or another coding agent:

```text
Install CodexSwitch CLI from https://github.com/syntaxskills/codexswitch-cli.
Prefer npm, verify it with `codexswitch-cli --version`, and tell me exactly
what changed. If npm is unavailable, install from the official Git repository
with Cargo.
```

### npm or Bun

```bash
npm install -g @syntaxskills/codexswitch-cli
# or
bun install -g @syntaxskills/codexswitch-cli
```

### Cargo

Install the current GitHub version:

```bash
cargo install --git https://github.com/syntaxskills/codexswitch-cli --locked
```

Requires Rust 1.94 or newer.

<details>
<summary>Prebuilt binary and crates.io</summary>

Install the latest prebuilt GitHub release:
```bash
curl -fsSL https://raw.githubusercontent.com/syntaxskills/codexswitch-cli/main/install.sh | bash
```

Or install the published crate with `cargo install codexswitch-cli --locked`.

</details>

## Quick Start

Log in to the first account and save it:

```bash
codex login
codexswitch-cli save --label work
```

Log in to another account and save that profile:

```bash
codex login
codexswitch-cli save --label personal
```

List and switch profiles:

```bash
codexswitch-cli list
codexswitch-cli load --label work
codexswitch-cli load --label personal
```

Run `load` without a selector to choose interactively.

### Provider-Specific Profiles

Use `--include-config` when an account also depends on settings in
`~/.codex/config.toml`, such as a custom provider:

```bash
codexswitch-cli save --label third-party --include-config
```

<details>
<summary>Manage only selected config keys</summary>

By default, a config-aware profile snapshots and restores the entire
`config.toml`. To preserve unrelated settings, create
`~/.codex/codexswitch/config.toml` and list the top-level keys CodexSwitch
should manage:

```toml
managed_config_keys = [
  "model",
  "model_provider",
  "model_providers",
]
```

</details>

## Why CodexSwitch

CodexSwitch saves named snapshots of Codex credentials and optional provider
settings, then restores them atomically.

| | |
| --- | --- |
| **Local-first** | Credentials stay on your machine; profile operations do not upload them. |
| **Config-aware** | Switch custom providers and models together with account credentials. |
| **Observable** | View account usage windows with `status`, including all saved profiles. |
| **Reliable** | Atomic writes, file locking, private permissions, diagnostics, and JSON output. |
| **Cross-platform** | Linux, macOS, and Windows, including Intel and Apple Silicon builds. |

## Commands

| Command | Purpose |
| --- | --- |
| `save --label <name>` | Save the active credentials as a named profile. |
| `load --label <name>` | Restore a profile; omit the selector for an interactive picker. |
| `list` | Show saved profiles and identify the active account. |
| `status --all` | Show usage windows for the active and saved profiles. |
| `label set`, `clear`, `rename` | Manage profile labels without re-saving credentials. |
| `export` / `import` | Back up or transfer profile bundles. |
| `doctor --fix` | Diagnose and repair local profile storage metadata. |
| `delete --label <name>` | Remove a saved profile with confirmation. |

Run `codexswitch-cli help <command>` for every option and example.

## Automation

Use `--json` for scripts and integrations:

```bash
codexswitch-cli list --json
codexswitch-cli status --all --json
```

Successful responses go to stdout. Failures use the same versioned envelope on
stderr and return a non-zero exit status. See the
[JSON output contract](docs/json-output.md).

## Storage and Security

Profiles are stored under:

```text
~/.codex/codexswitch/profiles/
```

> [!IMPORTANT]
> Saved auth files and export bundles contain secrets. Do not commit, publish,
> or share them. CodexSwitch keeps profile operations local, but `status` and
> token refreshes still contact the configured provider APIs.

Release binaries include SHA-256 checksums, manifests, and GitHub artifact
attestations. See [release verification](docs/verification.md).

## Documentation

- [Usage and command reference](docs/usage.md)
- [JSON output contract](docs/json-output.md)
- [Release verification](docs/verification.md)
- [Changelog](CHANGELOG.md)
- [Contributing](CONTRIBUTING.md)

## Project

SyntaxSkills maintains CodexSwitch as a community-owned open-source project.
Focused bug fixes, documentation improvements, portability work, and CLI
usability improvements are welcome.

CodexSwitch CLI is inspired by
[`codex-profiles`](https://github.com/midhunmonachan/codex-profiles) and
preserves its commit history in this repository.
