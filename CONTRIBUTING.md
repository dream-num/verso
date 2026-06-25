# Contributing

Thanks for taking the time to improve Verso.

## Local Setup

```sh
pnpm install
pnpm run check
```

Use Node.js 22.18.0 or newer. The repository runs TypeScript build scripts
directly through Node's native type stripping, CI currently runs Node.js 24, and
`.nvmrc` pins the recommended local major version. The root `.npmrc` enables
engine-strict installs so unsupported Node.js versions fail early. The root
`packageManager` field pins pnpm 11.9.0, and CI reads that version through
`pnpm/action-setup`.

Rust requires 1.85 or newer. The repository uses the stable toolchain with
rustfmt and clippy components, as declared in `rust-toolchain.toml`.

The check command is the same one used by CI. Pull requests run it on Linux,
macOS, and Windows so platform-specific path, shell, and packaging issues are
caught before release. It runs:

- `cargo fmt --check`
- `cargo clippy --locked --all-targets --all-features -- -D warnings`
- `cargo test --locked --all`
- TypeScript checks for release helper scripts
- TypeScript type checks for the npm wrapper
- wrapper tests

Pull requests that change dependency manifests or lockfiles run Dependency
Review before merge.

## Code Standards

Keep Rust code formatted with rustfmt and free of clippy warnings. Keep the
TypeScript wrapper strict-typecheck clean. Add or update tests for behavior
changes, especially release flow, rollback, versioning, and package boundary
changes.

Prefer small, direct changes that match the existing structure. Verso is meant
to stay focused, so new behavior should have a clear release-workflow use case.

Editor defaults are captured in `.editorconfig`; rustfmt and TypeScript remain
the source of truth for generated formatting.

## Package Topology

The npm wrapper and native platform packages are intentionally kept in one
workspace. When adding or removing a supported platform, update these files
together:

- `packages/verso/src/resolve.ts`
- `packages/verso/package.json`
- the matching `packages/verso-*` package manifest and README
- `.github/workflows/release.yml`
- `pnpm-workspace.yaml`

Keep `pnpm-workspace.yaml` `supportedArchitectures` in sync with the supported
platform packages. This keeps the optional dependency lockfile stable when CI
or contributors install from a different operating system or CPU architecture.

## Commit And Changelog Style

Verso's changelog is generated from Conventional Commits. Use subjects like:

- `feat(scope): summary` for user-facing features
- `fix(scope): summary` for bug fixes
- `perf(scope): summary` for performance improvements

Use `type(scope)!: summary` or a `BREAKING CHANGE:` footer for breaking changes.
Other conventional commit types are grouped under their own "Other Changes"
section. Non-conventional commits are ignored by the release notes and may lead
to a `No classifiable changes` entry when nothing else is releasable.

## Issues And Pull Requests

Use the bug report template for reproducible failures and the feature request
template for release-workflow proposals. For security concerns, follow
`SECURITY.md` and avoid public issues until the impact is understood.

For conduct concerns, follow `CODE_OF_CONDUCT.md`.

Pull requests should include a short summary and the verification performed.
For release behavior changes, include dry-run output or tests that cover the
planned file, changelog, git, and rollback effects.

## Publishing

Publishing is handled by GitHub Actions. Run the `Prepare Release` workflow
with a target version to update package versions, write the changelog, commit,
tag, and push. The pushed `v*` tag then triggers the `Release` workflow, which
builds binaries and publishes npm packages.

The repository must define `GH_TOKEN` with repository contents read/write access
and `NPM_TOKEN` with publish access for the `@univerkit` npm scope. The publish
workflow creates GitHub Release assets before publishing npm packages, publishes
platform packages before publishing the main `@univerkit/verso` wrapper, and
publish commands request npm provenance. The binary build matrix generates
GitHub Artifact Attestations for the native binaries. After trusted publishing
is configured for every published npm package, `NPM_TOKEN` can be removed from
the release workflow.

The current release workflow checks `NPM_TOKEN` before building release binaries
so missing publish credentials fail before platform build work starts.

Token-based publishing is the active workflow path. npm trusted publishing is a
migration target, not the active release path yet. Do not remove `NPM_TOKEN`
until this workflow has been changed and a tokenless publish has been tested
successfully.

Stable versions publish with the `latest` npm dist-tag. `alpha`, `beta`, and
`rc` prereleases publish with the matching dist-tag, so prerelease builds do not
replace the stable install channel. Stable GitHub Releases are marked as
latest. `alpha`, `beta`, and `rc` GitHub Releases are marked as prereleases and
are not promoted to latest. Rerunning the release workflow reapplies those
GitHub Release flags before replacing assets.

## Release Troubleshooting

Treat `Prepare Release` as the point where repository state changes. If it
fails before creating a release commit or tag, fix the reported check failure
and rerun the workflow with the same version. No registry or tag cleanup should
be needed.

If `Prepare Release` creates the local release commit and tag but cannot push
them, inspect the action logs first. After confirming the commit and tag are the
ones you want to publish, push them together with:

```sh
git push --follow-tags
```

The `Release` workflow is safe to rerun for the same tag. Reruns refresh the
GitHub Release notes and replace the binary assets. Platform packages are
published before the main wrapper package, and `scripts/publish-npm-packages.mts`
skips package versions that are already published so a partial npm publish can
continue from the remaining packages.

For binary asset problems, download the workflow artifact or GitHub Release
archive plus `SHA256SUMS.txt`, extract them, and verify the checksums before
debugging the npm wrapper:

```sh
shasum -a 256 -c SHA256SUMS.txt
```

When online, also verify the binary provenance with GitHub Artifact
Attestations:

```sh
gh attestation verify ./verso-linux-x64/verso \
  --repo dream-num/verso \
  --signer-workflow dream-num/verso/.github/workflows/release.yml
```
