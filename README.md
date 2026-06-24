# Verso

Verso is a small release CLI for JavaScript workspaces that publish multiple
packages at the same version. It updates package manifests, writes an
Angular-style conventional changelog, creates a release commit and tag, and
pushes with `git push --follow-tags`.

It is intentionally narrow: one config file, one release command, predictable
git output, and a dry run that shows exactly what would change.

## Installation

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

Create `verso.toml` in the project root. The only required field is
`workspaces.patterns`.

```toml
[workspaces]
patterns = [
  "apps/*",
  "bundle/*",
  "packages/*",
  "packages-experimental/*",
  "presets/packages/*",
]
```

The defaults are:

```toml
[version]
root_package = "package.json"
require_consistent_versions = true

[workspaces]
include_root = true

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

`changelog.preset` currently supports `angular` only. `git.push` currently
supports `follow-tags` only. GitHub release creation is reserved for a later
version, so `github_release.enabled = true` is rejected.

## CLI

```sh
pnpm release
pnpm release -- --dry-run
pnpm release -- --version 0.26.0
pnpm release -- --version 0.26.0 --yes
pnpm release -- --config path/to/verso.toml
pnpm release -- --help
```

Without `--version`, Verso prompts for patch, minor, major, alpha, beta, rc, or
custom semver. Exact versions can be passed with `--version`, including
prereleases such as `0.26.0-alpha.0`, `0.26.0-beta.1`, and `0.26.0-rc.2`.

Use `--config` to point at a different config file. Use `--yes` to skip the
confirmation shown when an explicit target version is not greater than the
current version. `--yes` does not choose a version for you; without `--version`,
interactive version selection still runs.

## What A Release Does

Verso reads the config, discovers matching `package.json` files, checks that
versions are consistent when configured to do so, resolves the target version,
updates the release package files, prepends `CHANGELOG.md`, commits, tags, and
pushes.

Dry runs do not write files or run mutating git commands. They print the
current version, target version, warnings, changelog path, planned git commands,
and a tree of package files that would be updated.

If a local release step fails, Verso makes a best-effort rollback of files it
modified, unstages release paths, and cleans up local release state where that
is safe. If the final push fails, the local release commit and tag are left in
place so you can fix the remote problem and run `git push --follow-tags`.
Rollback after a successful remote push is a manual operation.

## Distribution

`@univerkit/verso` is a TypeScript npm wrapper. The Rust binary is installed
from one of these optional platform packages:

- `@univerkit/verso-darwin-arm64`
- `@univerkit/verso-darwin-x64`
- `@univerkit/verso-linux-arm64`
- `@univerkit/verso-linux-x64`
- `@univerkit/verso-win32-x64`

The release workflow builds those binaries, copies them into their platform
packages, publishes the platform packages first, and then publishes
`@univerkit/verso`.

## Publishing

Releases are split into two GitHub Actions workflows.

`Prepare Release` is run manually with a target version. It updates package
versions, updates `CHANGELOG.md`, creates the release commit, creates the tag,
and pushes both back to the repository.

`Release` runs for `v*` tags and can also be run manually with a tag input. It
builds the platform binaries and publishes npm packages.

Configure these repository secrets before publishing:

- `RELEASE_TOKEN`: a token with repository contents read/write access. This is
  used by `Prepare Release` to push the release commit and tag.
- `NPM_TOKEN`: an npm token with publish access for the `@univerkit` scope.

## Development

```sh
pnpm install
pnpm run check
```

`pnpm run check` runs Rust formatting, clippy, Rust tests, TypeScript wrapper
checks, wrapper tests, and npm package dry-run checks.
