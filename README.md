<div align="center">

# CodexSwitch CLI

**Save once. Switch Codex accounts in one command.**

Fast, local-first profile switching for Codex, ChatGPT accounts, and custom providers.

[![Tests](https://github.com/syntaxskills/codexswitch-cli/actions/workflows/tests.yml/badge.svg?branch=main)](https://github.com/syntaxskills/codexswitch-cli/actions/workflows/tests.yml)
[![Docs](https://github.com/syntaxskills/codexswitch-cli/actions/workflows/docs.yml/badge.svg?branch=main)](https://github.com/syntaxskills/codexswitch-cli/actions/workflows/docs.yml)
[![Security audit](https://github.com/syntaxskills/codexswitch-cli/actions/workflows/security-audit.yml/badge.svg)](https://github.com/syntaxskills/codexswitch-cli/actions/workflows/security-audit.yml)
[![Release](https://github.com/syntaxskills/codexswitch-cli/actions/workflows/release.yml/badge.svg)](https://github.com/syntaxskills/codexswitch-cli/actions/workflows/release.yml)

[Quick start](#quick-start) · [Install](#install) · [Commands](#commands) · [Documentation](#documentation)

</div>

CodexSwitch stores named snapshots of `~/.codex/auth.json` and, when requested,
`~/.codex/config.toml`. It restores them atomically, so moving between work,
personal, and provider-specific profiles does not require copying credentials
by hand or repeatedly logging in.

```console
$ codexswitch-cli save --label work
Saved profile [PRO] work@example.com (work)
$ codexswitch-cli save --label personal
Saved profile [PRO] personal@example.com (personal)

$ codexswitch-cli list
[PRO] personal@example.com (personal) <- active · Credentials
[PRO] work@example.com (work) · Credentials

$ codexswitch-cli load --label work
Loaded profile [PRO] work@example.com (work)
```

## Highlights

| | |
| --- | --- |
| **Local by default** | Credentials stay on your machine. No account data is uploaded by CodexSwitch. |
| **Config-aware profiles** | Save credentials alone, or include provider and model settings from `config.toml`. |
| **Usage at a glance** | Inspect the active account, one saved profile, or every profile with `status`. |
| **Scriptable output** | Every command supports a versioned `--json` success and error envelope. |
| **Safe storage** | Atomic writes, file locking, private permissions, diagnostics, and repair tooling. |
| **Cross-platform** | Works on Linux, macOS, and Windows, including Intel and Apple Silicon builds. |

## Install

Install the current GitHub version with Cargo:
```bash
cargo install --git https://github.com/syntaxskills/codexswitch-cli --locked
```

This requires Rust 1.94 or newer.

<details>
<summary>Prebuilt binary, npm, Bun, and crates.io options</summary>

Install the latest prebuilt GitHub release:
```bash
curl -fsSL https://raw.githubusercontent.com/syntaxskills/codexswitch-cli/main/install.sh | bash
```

Install the published npm package:
```bash
npm install -g @syntaxskills/codexswitch-cli
# or
bun install -g @syntaxskills/codexswitch-cli
```

Install the published crate:

```bash
cargo install codexswitch-cli --locked
```

</details>

Verify the installation:

```bash
codexswitch-cli --version
```

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
