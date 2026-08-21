# Versioning Policy

## Semver Commitment

`rust-mcp-sdk` follows [Cargo SemVer](https://doc.rust-lang.org/cargo/reference/semver.html) strictly.

- **Major bumps (1.x → 2.x):** Reserved for intentional breaking changes to the public API. Deprecation warnings are issued one minor version prior when possible. A migration guide is provided with every major release.
- **Minor bumps (1.0 → 1.1):** New features, public API additions, new optional dependencies. No breaking changes.
- **Patch bumps (1.0.0 → 1.0.1):** Bug fixes, internal refactors, documentation improvements, dependency bumps with no public API impact.

## Workspace Crate Versioning

Each crate in the monorepo is independently versioned:

| Crate | Type | Versioning |
|-------|------|------------|
| `rust-mcp-sdk` | Core SDK | Own version |
| `rust-mcp-transport` | Transport layer | Own version |
| `rust-mcp-macros` | Proc-macro | Own version |
| `rust-mcp-extra` | Extensions | Own version |
| `rust-mcp-axum` | Axum integration | Own version |
| `rust-mcp-actix` | Actix-web integration | Own version |

Inter-crate dependency pins in the workspace root `Cargo.toml` are synchronized automatically by the release workflow. When a crate's public API changes, only that crate's version is bumped — downstream crates update their pin but not their own version unless their own public API changes.

## What Constitutes a Breaking Change

For the purposes of semver, the following are breaking changes:

- Removing or renaming public types, functions, methods, traits, or modules
- Changing function signatures (adding required parameters, changing types)
- Removing or renaming public trait methods
- Changing the variant set of a public enum
- Adding `#[non_exhaustive]` to a public struct or enum (prevents struct literals and exhaustive matching)
- Removing or renaming public feature flags
- Increasing the MSRV in a minor or patch release
- Changing the protocol version requirement in `rust-mcp-sdk`
- Removing or renaming re-exports

The following are **not** breaking changes:

- Adding new public types, functions, methods, or modules
- Adding new trait methods with default implementations
- Adding new enum variants to `#[non_exhaustive]` enums
- Adding new optional feature flags
- Internal refactors that don't affect the public API surface
- Dependency version bumps that don't affect the public API

## Deprecation Policy

1. A public API item is marked with `#[deprecated]` in a minor release, stating the replacement and removal target version.
2. The deprecated item remains functional for at least one full minor release cycle.
3. The item is removed in the next major release.

Example: An API deprecated in v1.2.0 will be removed no earlier than v2.0.0.

## Release Process

- **Patch releases:** Bug fixes, security patches, dependency updates.
- **Minor releases:** New features and public API additions.
- **Major releases:** Reserved for breaking changes, accompanied by a migration guide.

All releases are automated via [Release Please](https://github.com/googleapis/release-please) and published to [crates.io](https://crates.io/).

## MSRV Policy

The minimum supported Rust version (MSRV) is **1.80.0**. MSRV is only increased in minor or major releases when required by upstream dependencies or new language features. Any MSRV change includes a 30-day notice in the changelog.

## Pre-1.0 Stability

Versions prior to 1.0.0 (v0.x) followed Cargo's pre-1.0 semver compatibility rules: minor version bumps could include breaking changes. With the 1.0.0 release (2026-07-25), all crates are committed to the semver guarantee above.
