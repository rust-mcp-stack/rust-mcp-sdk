# Dependency Update Policy

## Rust MSRV

The minimum supported Rust version is **1.80.0**. New minor releases of the SDK will bump the MSRV only when required by major ecosystem upgrades, with a 30-day notice period.

## Direct Dependencies

- Dependencies are reviewed weekly via [Dependabot](https://docs.github.com/en/code-security/dependabot).
- Dependency updates follow [Conventional Commits](https://www.conventionalcommits.org/) with a `deps` prefix.
- Major version bumps are merged within 30 days of availability.
- All dependency upgrades run through the full CI pipeline (clippy, tests, doc-tests, format check, dependency audit) before merging.

## Security Patches

Security vulnerabilities in dependencies follow the response timeline defined in [SECURITY.md](../SECURITY.md):

- **Critical (P0):** resolved within 7 days
- **High (P1):** resolved within 14 days
- **Medium (P2):** resolved within 30 days
- **Low (P3):** resolved in the next release

Security advisories are published via [GitHub Security Advisories](https://github.com/rust-mcp-stack/rust-mcp-sdk/security/advisories).

## Semver Commitment

- **Major bumps (1.x → 2.x):** Reserved for intentional breaking changes. Deprecation warnings are issued one minor version prior when possible.
- **Minor bumps (1.0 → 1.1):** New features, public API additions. No breaking changes.
- **Patch bumps (1.0.0 → 1.0.1):** Bug fixes, internal refactors, dependency bumps with no public API impact.

Versioning follows [Cargo SemVer](https://doc.rust-lang.org/cargo/reference/semver.html) with the following guidelines for workspace crates:

| Crate | Follows |
|-------|---------|
| `rust-mcp-sdk` | Own version |
| `rust-mcp-transport` | Own version |
| `rust-mcp-macros` | Own version |
| `rust-mcp-extra` | Own version |
| `rust-mcp-axum` | Own version |
| `rust-mcp-actix` | Own version |

Inter-crate version pins in the workspace root `Cargo.toml` are updated automatically by the release workflow.

## External Schema Crate

`rust-mcp-schema` is a separate external crate maintained by the same organization. Updates to `rust-mcp-schema` that introduce protocol version changes are treated as major breaking changes in `rust-mcp-sdk`.
