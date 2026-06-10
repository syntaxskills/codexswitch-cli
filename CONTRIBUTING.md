# Contributing

Thanks for helping improve CodexSwitch CLI.

## Before You Start

**For non-trivial changes (new features, significant refactors, breaking changes):**

Please open an [issue](https://github.com/syntaxskills/codexswitch-cli/issues) or [discussion](https://github.com/syntaxskills/codexswitch-cli/discussions) first to:
- Discuss your proposed changes
- Get feedback on your approach
- Confirm the feature/fix aligns with project goals
- Avoid spending time on work that might not be accepted

**For minor changes (bug fixes, typos, docs improvements):**

Feel free to open a PR directly.

## What We're Looking For

**Contributions we welcome:**
- Bug fixes with test coverage
- Documentation improvements
- Performance optimizations
- New features that align with the project's scope (profile management)
- Test coverage improvements
- CI/CD enhancements

**Out of scope:**
- Features that duplicate Codex CLI functionality
- Changes that compromise security (token handling, HTTPS enforcement)
- Breaking changes without strong justification
- Features that significantly increase complexity

## Setup

- Rust toolchain: `rustup show`
- Node (for npm packaging)

## Checks

Run the same checks as the pre-commit hook:

```bash
make precommit
```

Other helpers:

```bash
make fmt
make clippy
make test
make coverage
```

## Pre-commit hook

Install the repo-managed hook wrapper (so updates are picked up automatically):

```bash
make hooks
```

This writes lightweight wrappers in your configured Git hooks directory
(respects `core.hooksPath`) that call the versioned hooks in `scripts/`
before each commit and push.

## Pull Request Guidelines

**Before submitting:**
- [ ] Run `make check` (or `make precommit`) - all checks must pass
- [ ] Add tests for new features or bug fixes
- [ ] Update documentation if behavior changes
- [ ] Keep commits focused and atomic
- [ ] Write clear commit messages

**PR description should include:**
- What problem does this solve?
- How does it solve it?
- Any breaking changes?
- Testing done (manual + automated)

**Review process:**
- Maintainers will review within a few days
- You may be asked to make changes
- Once approved, maintainers will merge

## Code Standards

- **Rust edition 2024** - follow existing patterns
- **90% minimum line coverage** - enforced via `make coverage`
- **No type suppression** - avoid `as any`, `#[allow]` without justification
- **Error handling** - proper `Result` types, no silent failures
- **Security-first** - especially around token/auth handling

## Release tag helper

Create a validated release tag that matches `Cargo.toml` and `package.json`:

```bash
make release-tag
```

To bump and tag in one step:

```bash
make release-tag ARGS="--bump patch"
```

`--bump` also syncs npm `optionalDependencies` package versions. `install.sh`
resolves the latest published release automatically, and you can still pin a
specific version with `CODEXSWITCH_CLI_VERSION` or `--version`.

## Questions?

Not sure if your idea fits? Ask in [Discussions](https://github.com/syntaxskills/codexswitch-cli/discussions) - we're happy to help!
