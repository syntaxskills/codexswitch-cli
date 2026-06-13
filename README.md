<h1 align="center">CodexSwitch CLI</h1>

<p align="center">Ultra-fast, cross-platform profile switching for ChatGPT, Codex, and third-party providers.</p>

<p align="center">
  <a href="https://github.com/syntaxskills/codexswitch-cli/actions/workflows/tests.yml"><img src="https://img.shields.io/github/actions/workflow/status/syntaxskills/codexswitch-cli/tests.yml?branch=main&label=tests" alt="Tests" /></a>
  <a href="https://github.com/syntaxskills/codexswitch-cli/releases"><img src="https://img.shields.io/github/v/release/syntaxskills/codexswitch-cli" alt="Release" /></a>
  <a href="https://github.com/syntaxskills/codexswitch-cli/stargazers"><img src="https://img.shields.io/github/stars/syntaxskills/codexswitch-cli?style=flat" alt="Stars" /></a>
  <a href="https://github.com/syntaxskills/codexswitch-cli/blob/main/LICENSE"><img src="https://img.shields.io/github/license/syntaxskills/codexswitch-cli?color=blue" alt="License" /></a>
</p>

## Install

Use npm or Bun if you want the easiest setup:

```bash
npm install -g @syntaxskills/codexswitch-cli
# or
bun install -g @syntaxskills/codexswitch-cli
```

Then verify the command is available:

```bash
codexswitch-cli --version
codexswitch-cli list
```

### Other Install Options

Install the latest GitHub release directly:

```bash
curl -fsSL https://raw.githubusercontent.com/syntaxskills/codexswitch-cli/main/install.sh | bash
```

Build from source:

```bash
cargo install --locked codexswitch-cli
```

Requires Rust 1.94 or newer.

## Setup

1. Log in with your first Codex account:

```bash
codex login
codexswitch-cli save --label work
```

2. Log in with another Codex account and save it:

```bash
codex login
codexswitch-cli save --label personal
```

3. Switch when needed:

```bash
codexswitch-cli list
codexswitch-cli load --label work
codexswitch-cli load --label personal
```

Use `--include-config` when a profile also needs `~/.codex/config.toml`, for
example custom providers:

```bash
codexswitch-cli save --label third-party --include-config
```

Then restore the official Codex provider configuration and re-save each official
profile with `--include-config`:

```bash
codexswitch-cli save --label work --include-config
codexswitch-cli save --label personal --include-config
```

By default, CodexSwitch backs up and replaces the entire `config.toml`. This is
the simplest behavior and ensures a profile restores exactly what was saved.

To save and replace only specific top-level fields while preserving everything
else in the active config, create `~/.codex/codexswitch/config.toml`:

```toml
managed_config_keys = [
  "model",
  "model_provider",
  "model_providers",
]
```

Loading an auth-only profile does not change any active config fields. Saving
provider-specific and official profiles with `--include-config` ensures that
switching profiles also switches providers correctly.

## Common Commands

| Command | What it does |
| --- | --- |
| `codexswitch-cli save --label work` | Save the current `~/.codex/auth.json` as a profile. |
| `codexswitch-cli load --label work` | Restore a saved profile. |
| `codexswitch-cli list` | Show saved profiles. |
| `codexswitch-cli status --all` | Show active and saved profile usage. |
| `codexswitch-cli export --output profiles.json` | Export profiles for backup or transfer. |
| `codexswitch-cli import --input profiles.json` | Import profiles from an export bundle. |
| `codexswitch-cli doctor --fix` | Check and repair local profile storage. |

Run `codexswitch-cli help <command>` for command-specific options.

## Storage

Profiles are stored locally under:

```text
~/.codex/codexswitch/profiles/
```

Auth files and export bundles contain secrets. Keep them private.

## Roadmap

- Polish the terminal UI for clearer, more attractive file-related output.
- Improve `status` with richer usage details, exploring the `codex-limit`
  methodology and ideas from
  [`codex-profiles` PR #24](https://github.com/midhunmonachan/codex-profiles/pull/24).

## Docs

- [Full usage guide](https://github.com/syntaxskills/codexswitch-cli/blob/main/docs/usage.md)
- [Release verification](https://github.com/syntaxskills/codexswitch-cli/blob/main/docs/verification.md)
- [Contributing](https://github.com/syntaxskills/codexswitch-cli/blob/main/CONTRIBUTING.md)

## About SyntaxSkills

SyntaxSkills hosts this repository to keep maintenance continuous and community-owned. The goal is not profit; it is to keep the tool maintained beyond any single person. Contributors and maintainers are welcome to join.
