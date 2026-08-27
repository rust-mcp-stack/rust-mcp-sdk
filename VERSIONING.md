# Versioning Policy

## Active lines

| Version | Protocol | Status |
|---|---|---|
| **2.x** | MCP 2026-07-28 | Current (beta → stable) |
| **1.x** | MCP 2025-11-25 | LTS (security fixes only) |

## 2.0 (2026-07-28)

Released as **2.0.0-beta.x** prereleases targeting the 2026-07-28 stateless
protocol.  No backwards-compatibility with 1.x.

> **Status (August 2026):** The pre-release channel is active. `2.0.0-beta.1` is
> the current beta. Server conformance (110/110) and client conformance (440/440)
> are passing on `--spec-version 2026-07-28`. A stable `2.0.0` is expected after
> community soak.

### Pre-release channel

- `2.0.0-beta.1` — initial beta
- `2.0.0-beta.N` — iteration on feedback
- `2.0.0` — stable (after conformance suite passes + community soak)

### Crate versions

| Crate | Version |
|---|---|
| `rust-mcp-schema` | `0.10.x` (see its own repo) |
| `rust-mcp-sdk` | `2.0.0-beta.x` → `2.0.0` |
| `rust-mcp-transport` | `2.0.0-beta.x` → `2.0.0` |
| `rust-mcp-macros` | `2.0.0-beta.x` → `2.0.0` |
| `rust-mcp-axum` | `2.0.0-beta.x` → `2.0.0` |
| `rust-mcp-actix` | `2.0.0-beta.x` → `2.0.0` |
| `rust-mcp-extra` | `2.0.0-beta.x` → `2.0.0` |

## 1.x LTS (`release-1.x` branch)

The **1.x** line targets MCP **2025-11-25** and receives **critical
security fixes** until at least **2027-07-28** (one year after the 2.0
stable release).

### Scope of LTS fixes

- Security vulnerabilities (CVEs)
- Build regressions on current stable Rust
- Critical protocol compliance gaps (verified via conformance suite)

### Out of scope

- New 2026-07-28 features
- Non-critical bug fixes
- Dependency upgrades beyond security

### Migration

See [`UPGRADING.md`] for the 1.x → 2.0 migration guide.

## Semver compliance

All crates follow [Semantic Versioning](https://semver.org).

- **Major** (X.0.0): breaking API changes (e.g. removed methods, changed
  signatures)
- **Minor** (0.X.0): new features, non-breaking additions
- **Patch** (0.0.X): bug fixes, performance improvements
