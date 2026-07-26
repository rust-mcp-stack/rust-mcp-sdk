# Roadmap


## Current State

- **Protocol Version**: 2025-11-25 (fully supported)
- **Server Conformance**: 40/40 scenarios passing (100%)
- **Client Conformance**: 254/254 scenarios passing (100%)
- **Crate Versions**: 1.0.x (stable; independently versioned - see [VERSIONING.md](VERSIONING.md))

---

## Spec Tracking

| Spec Version | Status |
|---|---|
| 2024-11-05 | Supported (backward compat) |
| 2025-03-26 | Supported (backward compat) |
| 2025-06-18 | Supported (backward compat) |
| 2025-11-25 | **Fully supported** (current stable) |
| 2026-07-28 (draft) | In progress |

---

## Spec Compliance — MCP 2026-07-28

**Primary goal.** Implement full support for the 2026-07-28 MCP specification and pass all corresponding conformance scenarios, including draft-spec suites.

**Target:** a stable release with 2026-07-28 support within 30 days of the spec publish date.

Scope (all draft SEPs, aligned with what the conformance suite requires):

- [ ] Stateless lifecycle (`server/discover`, per-request `_meta`)
- [ ] SEP-2575: Removed methods (initialize, ping, etc.) return 404
- [ ] SEP-2243: HTTP standard/custom header validation
- [ ] SEP-2322: InputRequiredResult / MRTR (14 conformance scenarios)
- [ ] SEP-2549: Caching hints (`ttlMs`, `cacheScope`)
- [ ] SEP-2663: Tasks extension
- [ ] Pass all draft-spec conformance scenarios in CI (`--spec-version draft`)

---

## SDK Improvements

- [ ] Per-request notification context (replace `task_local!` with a handler-trait extension)
- [ ] JSON Schema 2020-12 tool support (`json_schema_2020_12_tool` for SEP-1613/2106)
- [ ] SSE polling / reconnection support (SEP-1699)
- [ ] Dynamic tool/resource/prompt registration with list-changed notifications
- [ ] Async runtime agnostic (long-term) - decouple from Tokio so the SDK can run on alternative runtimes (e.g., `async-std`, `smol`) and in constrained environments

---

## Production Hardening

- [ ] Comprehensive error-handling documentation
- [ ] Performance benchmarks vs TypeScript/Python SDKs
- [ ] Connection pooling and backpressure tuning
- [x] Metrics/observability integration

---

## Ecosystem & Platforms

- [ ] Additional HTTP server backends out of the box (beyond Axum and Actix)
- [ ] Cloudflare Workers support
- [ ] Project template / `cargo-generate` scaffold

---

## Extensions (`rust-mcp-extra`)

Expand the catalog of ready-to-use, pluggable implementations:

- [ ] Additional auth providers (beyond Keycloak, WorkOS AuthKit, Scalekit)
- [ ] Additional ID generators
- [ ] Additional token verifiers
- [ ] Additional event stores (resumability) and session stores (beyond in-memory and SQLite)

---



## Dependency Updates

See [docs/dependency-policy.md](docs/dependency-policy.md).
