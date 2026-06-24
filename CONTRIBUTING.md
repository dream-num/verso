# Contributing

Thanks for taking the time to improve Verso.

## Local Setup

```sh
pnpm install
pnpm run check
```

The check command is the same one used by CI. It runs:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- TypeScript type checks for the npm wrapper
- wrapper tests
- npm package dry-run checks

## Code Standards

Keep Rust code formatted with rustfmt and free of clippy warnings. Keep the
TypeScript wrapper strict-typecheck clean. Add or update tests for behavior
changes, especially release flow, rollback, versioning, and package boundary
changes.

Prefer small, direct changes that match the existing structure. Verso is meant
to stay focused, so new behavior should have a clear release-workflow use case.

## Publishing

Publishing is handled by GitHub Actions. Push a `v*` tag or run the `Release`
workflow manually with a tag input.

The repository must define `NPM_TOKEN` with publish access for the `@univerkit`
npm scope. The workflow publishes platform packages before publishing the main
`@univerkit/verso` wrapper.
