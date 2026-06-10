<h1 align="center">CodexSwitch CLI</h1>

<p align="center">Seamlessly switch between multiple Codex accounts</p>

<p align="center">
  <a href="https://github.com/syntaxskills/codexswitch-cli/actions/workflows/tests.yml"><img src="https://img.shields.io/github/actions/workflow/status/syntaxskills/codexswitch-cli/tests.yml?branch=main&label=tests" alt="Tests" /></a>
  <a href="https://github.com/syntaxskills/codexswitch-cli/releases"><img src="https://img.shields.io/github/v/release/syntaxskills/codexswitch-cli" alt="Release" /></a>
  <a href="https://github.com/syntaxskills/codexswitch-cli/stargazers"><img src="https://img.shields.io/github/stars/syntaxskills/codexswitch-cli?style=flat" alt="Stars" /></a>
  <a href="https://github.com/syntaxskills/codexswitch-cli/blob/main/LICENSE"><img src="https://img.shields.io/github/license/syntaxskills/codexswitch-cli?color=blue" alt="License" /></a>
</p>

<p align="center">
  <a href="#overview">Overview</a> •
  <a href="#install">Install</a> •
  <a href="#usage">Usage</a> •
  <a href="#more-docs">More Docs</a> •
  <a href="#faq">FAQ</a>
</p>

---

## Overview

CodexSwitch CLI lets you save and switch easily between multiple Codex accounts without repeated `codex login`

CodexSwitch CLI is an actively maintained, MIT-licensed fork of
[`midhunmonachan/codex-profiles`](https://github.com/midhunmonachan/codex-profiles).
The original copyright and license notice are preserved in this repository.

## Install

<table width="100%">
  <colgroup>
    <col style="width: 25%;" />
    <col style="width: 75%;" />
  </colgroup>
  <thead>
    <tr>
      <th align="left">Method</th>
      <th align="left" style="white-space: nowrap;">Command</th>
    </tr>
  </thead>
  <tbody>
    <tr>
      <td>npm</td>
      <td style="white-space: nowrap;"><code>npm install -g codexswitch-cli</code></td>
    </tr>
    <tr>
      <td>Bun</td>
      <td style="white-space: nowrap;"><code>bun install -g codexswitch-cli</code></td>
    </tr>
  </tbody>
</table>

### Manual install

```bash
curl -fsSL https://raw.githubusercontent.com/syntaxskills/codexswitch-cli/main/install.sh | bash
```

<details>
<summary>Advanced install option (build from source)</summary>

```bash
cargo install --locked codexswitch-cli
```

Requires Rust 1.94+

</details>

## Quick Start

```bash
codexswitch-cli save --label work
codexswitch-cli save --label third-party --include-config
codexswitch-cli list
codexswitch-cli load --label work --force
```

The npm package also installs `codexswitch` and `codex-profiles` command aliases.

## Usage

> [!NOTE]
> CodexSwitch CLI data is stored under `~/.codex/profiles/` on your machine

### Command Reference

<table width="100%">
  <thead>
    <tr>
      <th align="left" width="44%">Command</th>
      <th align="left">Description</th>
    </tr>
  </thead>
  <tbody>
    <tr>
      <td width="43%"><code>codexswitch-cli save</code><br/><code>[--label &lt;name&gt;]</code><br/><code>[--include-config]</code></td>
      <td>Save current <code>auth.json</code><br/>Optionally include <code>config.toml</code> for provider-specific profiles</td>
    </tr>
    <tr>
      <td width="43%"><code>codexswitch-cli load</code><br/><code>(--label &lt;name&gt; | --id &lt;profile-id&gt;)</code><br/><code>[--force]</code></td>
      <td>Load a saved profile<br/>Choose a target profile and force when needed</td>
    </tr>
    <tr>
      <td width="43%"><code>codexswitch-cli list</code><br/><code>[--show-id] [--json]</code></td>
      <td>List profiles<br/>Supports id and JSON views</td>
    </tr>
    <tr>
      <td width="44%"><code>codexswitch-cli export</code><br/><code>[--label &lt;name&gt;]</code><br/><code>[--id &lt;profile-id&gt; (repeatable)]</code></td>
      <td>Export to a JSON bundle<br/>Default: all profiles, or a selected subset</td>
    </tr>
    <tr>
      <td width="43%"><code>codexswitch-cli import</code><br/><code>--input &lt;file&gt;</code></td>
      <td>Import from a JSON bundle</td>
    </tr>
    <tr>
      <td width="43%"><code>codexswitch-cli doctor</code><br/><code>[--fix] [--json]</code></td>
      <td>Run diagnostics and optionally apply safe repairs</td>
    </tr>
    <tr>
      <td width="43%"><code>codexswitch-cli label set</code><br/><code>(--label &lt;name&gt; | --id &lt;profile-id&gt;)</code><br/><code>--to &lt;label&gt;</code></td>
      <td>Set or replace a label<br/>Target one profile</td>
    </tr>
    <tr>
      <td width="43%"><code>codexswitch-cli label clear</code><br/><code>(--label &lt;name&gt; | --id &lt;profile-id&gt;)</code></td>
      <td>Clear a label<br/>Target one profile</td>
    </tr>
    <tr>
      <td width="43%"><code>codexswitch-cli label rename</code><br/><code>--label &lt;label&gt; --to &lt;label&gt;</code></td>
      <td>Rename an existing label</td>
    </tr>
    <tr>
      <td width="44%"><code>codexswitch-cli status</code><br/><code>[--label &lt;name&gt; | --id &lt;profile-id&gt;]</code><br/><code>[--all] [--json]</code></td>
      <td>Show usage for active, selected, or all targets<br/>Human-readable or JSON output</td>
    </tr>
    <tr>
      <td width="44%"><code>codexswitch-cli delete</code><br/><code>[--label &lt;name&gt; | --id &lt;profile-id&gt; (repeatable)]</code><br/><code>[--yes]</code></td>
      <td>Delete by label or id<br/>Supports bulk delete and non-interactive mode</td>
    </tr>
  </tbody>
</table>

### Notes

- `load` and `delete` are interactive unless you pass `--label` or `--id`
- Profiles are auth-only by default: saving without `--include-config` stores and restores only `auth.json`
- Use `save --include-config` for third-party or custom-provider profiles that also need `~/.codex/config.toml`
- `list`, `status`, and JSON output show `managed_files`, for example `auth.json` or `auth.json + config.toml`
- Export bundles contain secrets

## More Docs

- [Release verification guide](https://github.com/syntaxskills/codexswitch-cli/blob/main/docs/verification.md)
- [Contribution guide](https://github.com/syntaxskills/codexswitch-cli/blob/main/CONTRIBUTING.md)

## FAQ

<details>
<summary>How do I uninstall?</summary>

> - npm: `npm uninstall -g codexswitch-cli`
> - Bun: `bun uninstall -g codexswitch-cli`
> - Cargo: `cargo uninstall codexswitch-cli`
> - Manual: `rm ~/.local/bin/codexswitch-cli`
</details>

<details>
<summary>Is my auth file uploaded anywhere?</summary>

> No. Everything stays on your machine. This tool only copies files locally
</details>

<details>
<summary>What is a “profile” in this tool?</summary>

> A profile is a saved copy of your `~/.codex/auth.json`. Each profile represents
> one Codex account
</details>

<details>
<summary>What happens if I run load without saving?</summary>

> You will be prompted to save the active profile, continue without saving, or
> cancel
</details>

<details>
<summary>Can I keep personal and work accounts separate?</summary>

> Yes. Save each account with a label (for example, `personal` and `work`) and
> switch with the label
</details>

<details>
<summary>How can I verify my installation?</summary>

> ```bash
> codexswitch-cli --version
> codexswitch-cli list
> ```
</details>

<details>
<summary>Where are release verification steps documented?</summary>

> In [docs/verification.md](https://github.com/syntaxskills/codexswitch-cli/blob/main/docs/verification.md)
</details>
