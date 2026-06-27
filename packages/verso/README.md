# @univerkit/verso

Verso is a focused release CLI for JavaScript packages and workspaces that
publish related packages at the same version. It updates package manifests, writes an
Angular-style changelog, creates a release commit and tag, and pushes with
`git push --follow-tags`.

This npm package is the JavaScript wrapper for the native Verso binary. The
matching binary is installed through an optional platform package such as
`@univerkit/verso-darwin-arm64`, `@univerkit/verso-linux-x64`, or
`@univerkit/verso-win32-x64`.

## Supported Platforms

| Platform | CPU | Package |
| --- | --- | --- |
| macOS | arm64 | `@univerkit/verso-darwin-arm64` |
| macOS | x64 | `@univerkit/verso-darwin-x64` |
| Linux | arm64 | `@univerkit/verso-linux-arm64` |
| Linux | x64 | `@univerkit/verso-linux-x64` |
| Windows | x64 | `@univerkit/verso-win32-x64` |

## Installation

Requires Node.js 22.18.0 or newer.

```sh
pnpm add -D @univerkit/verso
```

Add a release script:

```json
{
  "scripts": {
    "release": "verso"
  }
}
```

Single-package projects can run without `verso.toml`. When the default
`verso.toml` is missing and `package.json` exists, Verso releases the root
package with built-in defaults.

Create `verso.toml` only when you need to customize behavior. Single-package
projects can start with:

```toml
[version]
root_package = "package.json"
```

Workspace projects can add package globs:

```toml
[workspaces]
patterns = ["packages/*"]
```

Then run:

```sh
pnpm release
pnpm release -- --dry-run
pnpm release -- --dry-run --json
pnpm release -- --version 1.2.3 --yes
pnpm release -- init
pnpm release -- doctor
pnpm release -- -V
```

## Troubleshooting

If running `verso` prints `Could not find Verso platform binary`, the native
optional dependency for your operating system was not installed or is not
available for your platform.

Check these first:

- install `@univerkit/verso`, not a platform package directly
- make sure optional dependencies are enabled in your package manager
- confirm your machine is one of the supported platform and CPU pairs above
- reinstall from a fresh lockfile if the lockfile was created on a different
  operating system or with optional dependencies disabled

If running `verso` prints `Failed to launch Verso binary`, the platform package
was found but the native executable could not be started. On macOS and Linux,
the wrapper repairs missing executable bits before spawning the native binary.
Upgrade `@univerkit/verso`, reinstall dependencies, and include the error's
cause line in bug reports if the failure continues.

An unsupported platform needs a new native package before the wrapper can run.
For bug reports, include the output of `pnpm release -- -V`. The wrapper handles
that command before loading the native binary, so it still works when the
platform optional dependency is missing.

See the repository README for the full configuration reference and release
workflow details: https://github.com/dream-num/verso
