# Breaking Changes

## Policy

`rust-mcp-sdk` is committed to stability and follows [Cargo SemVer](https://doc.rust-lang.org/cargo/reference/semver.html). Breaking changes are reserved for major version bumps (1.x → 2.x).

When a breaking change is necessary:

1. The affected API is marked `#[deprecated]` in a minor release with the migration path documented.
2. The deprecated API remains functional for at least one full minor release cycle.
3. A migration guide is published in `doc/migration/` alongside the major release.
4. Breaking changes are documented in `CHANGELOG.md` and in per-crate changelogs.

## Historical Breaking Changes

### v1.0.0 (2026-07-25)

First stable release. Breaking changes from v0.10.x:

- Typo fixes in public API:
  - `ensure_server_protocole_compatibility` → `ensure_server_protocol_compatibility`
  - `AuthMetadateError` → `AuthMetadataError`
  - `oauth_endppoints()` → `oauth_endpoints()`
  - `reqquired_scopes()` → `required_scopes()`
- Removed `rust_mcp_transport::utils::http_utils::get_header_value()` — use `response.headers().get()` directly
- Removed commented-out dead code (`IntoClientTransport`, `AxumServer::with_layer`)

Migration guide: [doc/migration/v0.10.x-to-v1.0.0.md](doc/migration/v0.10.x-to-v1.0.0.md)

### v0.10.0 (2026-06-24)

Architectural refactor — HTTP frameworks extracted from core SDK:

- `hyper_server` module removed from `rust-mcp-sdk`
- Users must now add `rust-mcp-axum` (or `rust-mcp-actix`) as a separate dependency
- `HyperServerOptions` renamed to `AxumServerOptions`
- BYO-server route imports changed (`mcp_routes` moved to `rust-mcp-axum`)
- `McpAppState::handle_mcp_request` now returns framework-agnostic `McpHttpError` instead of Axum-specific response types
- `McpMountOptions` introduced for shared HTTP configuration

Migration guide: [doc/migration/v0.9.x-to-v0.10.x.md](doc/migration/v0.9.x-to-v0.10.x.md)

### Earlier Versions

For breaking changes in v0.8.x and earlier (all pre-stability), see the [full changelog](CHANGELOG.md). Pre-1.0 versions followed Cargo's pre-1.0 semver rules where minor versions could include breaking changes.

## Future Breaking Changes

No breaking changes are currently planned. If a breaking change becomes necessary, it will be announced in advance: the affected API will be deprecated at least one minor release prior to removal, and a migration guide will be published alongside the major release.
