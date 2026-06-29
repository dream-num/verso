# Verso

[English](README.md) | [简体中文](README.zh-CN.md)

[![CI](https://github.com/dream-num/verso/actions/workflows/ci.yml/badge.svg)](https://github.com/dream-num/verso/actions/workflows/ci.yml)
[![npm version](https://img.shields.io/npm/v/@univerkit/verso.svg)](https://www.npmjs.com/package/@univerkit/verso)
[![license](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Verso is a small release CLI for JavaScript workspaces that publish multiple
packages at the same version. It updates package manifests, writes an
Angular-style conventional changelog, creates a release commit and tag, and
pushes with `git push --follow-tags`.

## Installation

Verso requires Node.js 22.18.0 or newer.

```sh
pnpm add -D @univerkit/verso
```

Add a release script to your package manifest:

```json
{
  "scripts": {
    "release": "verso"
  }
}
```

## Configuration

Single-package projects can run without `verso.toml`. When the default
`verso.toml` is missing and a root package manifest exists, Verso uses built-in
defaults and releases the root package.

Create `verso.toml` only when you need to customize behavior. For a
single-package project, it can be minimal:

```toml
[version]
root_package = "package.json"
```

For a workspace release, configure package globs:

```toml
[workspaces]
patterns = [
  "apps/*",
  "examples/**",
  "bundle/*",
  "packages/*",
  "!packages/**/fixtures",
  "packages-experimental/*",
  "presets/packages/*",
]
```

If `workspaces.patterns` is omitted, Verso reads package manager workspace
metadata before falling back to single-package mode. It supports
`pnpm-workspace.yaml` `packages`, root manifest `workspaces: ["packages/*"]`,
and root manifest `workspaces: { "packages": ["packages/*"] }`.

Package discovery supports `package.json`, `package.json5`, `package.yaml`, and
`package.yml`. When multiple manifest files exist in the same directory, Verso
uses that order.

The defaults are:

```toml
[version]
root_package = "package.json"
require_consistent_versions = true
cargo_manifest_paths = []

[workspaces]
include_root = true
ignore = []
use_gitignore = true

[changelog]
infile = "CHANGELOG.md"
preset = "angular"

[git]
require_clean_worktree = true
commit_message = "chore(release): release v${version}"
tag_name = "v${version}"
push = "follow-tags"

[github_release]
enabled = false
```

Hooks are optional and default to disabled:

```toml
[hooks]
before_version = "pnpm test"
after_version = "pnpm build"
before_commit = "pnpm lint"
after_push = "node scripts/notify-release.mts"
```

`changelog.preset` currently supports `angular` only. `git.push` currently
supports `follow-tags` only. The CLI does not create GitHub Releases from
project configs yet, so `github_release.enabled = true` is rejected.

### Configuration Reference

When `--config` is omitted and `verso.toml` is missing, Verso falls back to the
built-in defaults only if a root package manifest exists. Explicit
`--config <PATH>` values are always required to exist.

| Key | Required | Default | Notes |
| --- | --- | --- | --- |
| `workspaces.patterns` | No | `[]` | Package workspace glob patterns relative to the config directory. Use forward slashes. Supports `*`, `**`, `?`, character classes, braces, and `!` exclusions. When omitted, Verso reads `pnpm-workspace.yaml` or root manifest `workspaces`; if neither exists, it uses single-package mode. |
| `workspaces.include_root` | No | `true` | Include the root package selected by `version.root_package`. |
| `workspaces.ignore` | No | `[]` | Extra workspace discovery ignore patterns. Plain path segments such as `fixtures` match directories by name. |
| `workspaces.use_gitignore` | No | `true` | Respect root and nested `.gitignore` files during workspace discovery. |
| `version.root_package` | No | `package.json` | Package manifest used for the current version and root update. Use forward slashes; must stay under the config directory. If omitted and `package.json` is absent, Verso looks for `package.json5`, `package.yaml`, then `package.yml`. |
| `version.require_consistent_versions` | No | `true` | Fail when discovered packages or configured Cargo manifests do not share one version. |
| `version.cargo_manifest_paths` | No | `[]` | Cargo manifests under the config directory whose `[package].version` should be updated. Use forward slashes. The nearest `Cargo.lock` is updated when present. |
| `changelog.infile` | No | `CHANGELOG.md` | Changelog file prepended during release. Use forward slashes; must stay under the config directory. |
| `changelog.preset` | No | `angular` | Only `angular` is supported. |
| `git.require_clean_worktree` | No | `true` | Require a clean worktree before mutating files. |
| `git.commit_message` | No | `chore(release): release v${version}` | `${version}` is replaced with the target version. Must not be empty. |
| `git.tag_name` | No | `v${version}` | Must contain `${version}` and render a valid Git tag. |
| `git.push` | No | `follow-tags` | Only `follow-tags` is supported. |
| `github_release.enabled` | No | `false` | `true` is rejected in this version. |
| `hooks.before_version` | No | None | Shell command run before release files are updated. |
| `hooks.after_version` | No | None | Shell command run after release files are updated. |
| `hooks.before_commit` | No | None | Shell command run before staging and committing. |
| `hooks.after_commit` | No | None | Shell command run after the release commit is created. |
| `hooks.before_tag` | No | None | Shell command run before the release tag is created. |
| `hooks.after_tag` | No | None | Shell command run after the release tag is created. |
| `hooks.before_push` | No | None | Shell command run before `git push --follow-tags`. |
| `hooks.after_push` | No | None | Shell command run after the push succeeds. |

## CLI

```sh
pnpm release
pnpm release -- --dry-run
pnpm release -- --version 0.26.0
pnpm release -- --version 0.26.0 --yes
pnpm release -- --dry-run --json
pnpm release -- --config path/to/verso.toml
pnpm release -- doctor
pnpm release -- init
pnpm release -- -V
pnpm release -- --help
```

| Option | Default | Description |
| --- | --- | --- |
| `--dry-run` | `false` | Preview the release without writing files or running mutating git commands. |
| `--json` | `false` | Print dry-run output as JSON. Must be used with `--dry-run`. |
| `--version <SEMVER>` | None | Use an exact target version without interactive selection. |
| `--config <PATH>` | `verso.toml` | Read a different config file. |
| `--yes` | `false` | Skip release confirmation prompts. It does not choose a version. |
| `-V, --tool-version` | None | Print the Verso CLI version. |
| `--help` | None | Print CLI help. |

Subcommands:

| Command | Description |
| --- | --- |
| `verso init` | Create a starter `verso.toml`. It auto-detects `packages/*`; use `--single`, `--workspace`, or `--force` to override behavior. |
| `verso doctor` | Validate config parsing, package discovery, version consistency, changelog path, and Cargo manifest versions. Use `--json` for structured output. |

Without `--version`, Verso opens an interactive menu for patch, minor, major,
alpha, beta, rc, or custom semver. Prerelease channels then prompt for a base
version choice, including a custom base version. Exact versions can be passed
with `--version`, including prereleases such as `0.26.0-alpha.0`,
`0.26.0-beta.1`, and `0.26.0-rc.2`.

Use `--config` to point at a different config file. Use `--yes` to skip release
confirmation prompts, including the confirmation shown when an explicit target
version is not greater than the current version. `--yes` does not choose a
version for you; without `--version`, interactive version selection still runs.
Use `-V` or `--tool-version` to print the installed Verso CLI version without
reading release config.

When stdin or stdout is not attached to a terminal, Verso keeps a plain text
prompt fallback so scripted tests and piped input can continue to choose by
name, such as `beta` followed by `minor`.

## What A Release Does

Verso reads the config, discovers matching package manifests, checks that
versions are consistent when configured to do so, and resolves the target
version. During a real release, it asks for confirmation before updating release
files, committing, tagging, and pushing. These confirmations default to yes:
press Enter to continue, or answer `n` to stop before the next step. Passing
`--yes` skips those confirmations. Updating release files changes package files,
any configured Cargo manifests, and their nearest `Cargo.lock` files when
present, and prepends `CHANGELOG.md`.

Dry runs do not write files or run mutating git commands. They print the
current version, target version, warnings, changelog path, planned git commands,
planned hooks, and a tree of version files that would be updated. Dry runs list
hooks but do not execute them.

`--dry-run --json` prints the same release plan as structured JSON for scripts
and CI systems.

Workspace discovery always skips `.git` and `node_modules`. By default it also
respects root and nested `.gitignore` files, so ignored directories are not
scanned for release packages. Set `workspaces.use_gitignore = false` if a
project intentionally publishes packages from ignored directories. Verso updates
package manifest versions only; it does not rewrite workspace dependency ranges
or run package-manager publish commands.

If a local release command fails, Verso makes a best-effort rollback of files it
modified, unstages release paths, and cleans up local release state where that
is safe. If you answer `n` to a release confirmation, Verso stops without
rolling back already completed steps. If the final push fails, the local release
commit and tag are left in place so you can fix the remote problem and run
`git push --follow-tags`. Rollback after a successful remote push is a manual
operation.

Maintainer development and publishing details live in
[CONTRIBUTING.md](CONTRIBUTING.md).
