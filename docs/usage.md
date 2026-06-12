# Usage Guide

This page contains the detailed command reference for CodexSwitch CLI.

## Profile Setup

Save the currently logged-in Codex account:

```bash
codexswitch-cli save --label work
```

Save both `auth.json` and `config.toml` for provider-specific profiles:

```bash
codexswitch-cli save --label third-party --include-config
```

Switch to a saved profile:

```bash
codexswitch-cli load --label work
```

`load` and `delete` are interactive unless you pass `--label` or `--id`.

## Command Reference

| Command | Description |
| --- | --- |
| `codexswitch-cli save [--label <name>] [--include-config]` | Save the current `auth.json`. Add `--include-config` to save `config.toml` too. |
| `codexswitch-cli load (--label <name> \| --id <profile-id>) [--force]` | Load a saved profile. Use `--force` to continue without saving the active profile first. |
| `codexswitch-cli list [--show-id] [--json]` | List profiles. JSON output always includes ids. |
| `codexswitch-cli export --output <file> [--label <name> \| --id <profile-id>]` | Export all profiles, or one selected profile, to a JSON bundle. `--id` is repeatable. |
| `codexswitch-cli import --input <file>` | Import profiles from a JSON bundle. Fails on id or label conflicts. |
| `codexswitch-cli doctor [--fix] [--json]` | Run diagnostics and optionally repair profile storage metadata. |
| `codexswitch-cli label set (--label <name> \| --id <profile-id>) --to <label>` | Set or replace a label. |
| `codexswitch-cli label clear (--label <name> \| --id <profile-id>)` | Clear a label. |
| `codexswitch-cli label rename --label <label> --to <label>` | Rename an existing label. |
| `codexswitch-cli status [--label <name> \| --id <profile-id> \| --all] [--json]` | Show usage for the active profile, one saved profile, or all saved profiles. |
| `codexswitch-cli delete [--label <name> \| --id <profile-id>] [--yes]` | Delete by label or id. `--id` is repeatable. |

Global options:

```bash
codexswitch-cli --json <command>
codexswitch-cli --plain <command>
```

## Storage Notes

- Profiles are auth-only by default.
- Saved profiles use `~/.codex/codexswitch/profiles/<profile-id>/auth.json`.
- Profiles saved with `--include-config` also store `config.toml` in the same folder.
- By default, loading a config profile replaces the entire active `config.toml`.
- Set `managed_config_keys` in `~/.codex/codexswitch/config.toml` to save and replace only selected top-level fields while preserving everything else.
- `list`, `status`, and JSON output show `managed_files`, such as `auth.json` or `auth.json + config.toml`.
- Export bundles contain secrets.

## Uninstall

```bash
npm uninstall -g @syntaxskills/codexswitch-cli
bun uninstall -g @syntaxskills/codexswitch-cli
cargo uninstall codexswitch-cli
rm ~/.local/bin/codexswitch-cli
```

## FAQ

### Is my auth file uploaded anywhere?

No. Everything stays on your machine. This tool only copies files locally.

### What is a profile in this tool?

A profile is a saved copy of your `~/.codex/auth.json`. Each profile represents one Codex account.

### What happens if I run load without saving?

You will be prompted to save the active profile, continue without saving, or cancel.

### Can I keep personal and work accounts separate?

Yes. Save each account with a label, for example `personal` and `work`, then switch with `load --label`.
