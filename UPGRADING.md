# Upgrading rust-mcp-sdk

This document provides links to migration guides for upgrading across breaking versions of `rust-mcp-sdk`.

We strive to keep the SDK backward-compatible where possible, but given the rapid evolution of the Model Context Protocol (MCP), breaking changes are occasionally necessary to support new architectures and features.

If you are upgrading between major/breaking versions, please consult the guides below:

## Migration Guides

- **[v1.x to v2.0.0](doc/migration/v1.x-to-v2.0.0.md)** (Migration to MCP 2026-07-28 stateless protocol — initialize handshake, task system, and sessions removed)
- **[v0.10.x to v1.0.0](doc/migration/v0.10.x-to-v1.0.0.md)** (First stable release — typo fixes, dead code removal, handler traits stabilized)
- **[v0.9.x to v0.10.x](doc/migration/v0.9.x-to-v0.10.x.md)** (Extraction of Axum/Actix HTTP frameworks into standalone crates)

---

*Note: For minor version bumps (e.g., v0.9.0 to v0.9.1), no migration steps are typically required. Please consult the standard GitHub Release Notes for details on features and bug fixes.*
/Users/ahashemi/.cargo-target/rust-mcp-sdk/target/release/examples/hello-world-mcp-server-stdio