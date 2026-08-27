# Roadmap


## Current State

- **Protocol Version**: 2026-07-28 (fully supported)
- **Server Conformance**: 110/110 scenarios passing (100%)
- **Client Conformance**: 440/440 scenarios passing (100%)
- **Crate Versions**: 2.0.0 (independently versioned - see [VERSIONING.md](VERSIONING.md))

---

## Spec Tracking

| Spec Version | Status |
|---|---|
| 2026-07-28 | **Fully supported** (current) |
| 2025-11-25 | LTS on the `release-1.x` branch (no longer on `main`) |
| ≤ 2025-06-18 | Not supported on `main` (see 1.x history) |

---

## Spec Compliance — MCP 2026-07-28

**Completed.** Full support for the 2026-07-28 MCP specification with all conformance scenarios passing.

- [x] Stateless lifecycle (`server/discover`, per-request `_meta`)
- [x] SEP-2575: Stateless request lifecycle (per-request `_meta` gate, HTTP status mapping)
- [x] SEP-2243: HTTP standard/custom header validation
- [x] SEP-2322: InputRequiredResult / MRTR
- [x] SEP-2549: Caching hints (`ttlMs`, `cacheScope`) — response cache with principal-scoped privacy
- [x] Pass all conformance scenarios in CI (`--spec-version 2026-07-28`)

---

## SDK Improvements

- [ ] Per-request notification context (replace `task_local!` with a handler-trait extension)
- [ ] JSON Schema 2020-12 tool support (`json_schema_2020_12_tool` for SEP-1613/2106)
- [ ] Dynamic tool/resource/prompt registration with list-changed notifications
- [ ] Async runtime agnostic (long-term) - decouple from Tokio so the SDK can run on alternative runtimes (e.g., `async-std`, `smol`) and in constrained environments
- **Deprecated:** `logLevel` (`_meta.logLevel`) is soft-deprecated per SEP-2577. Roots and Sampling remain as first-class `ClientCapabilities` (`roots` / `sampling`, enforced per-request via `RequiredClientCapability`) delivered through MRTR. Their standalone server→client request methods were removed.

---

## Production Hardening

- [ ] Comprehensive error-handling documentation
- [ ] Performance benchmarks vs TypeScript/Python SDKs
- [ ] Connection pooling and backpressure tuning
- [x] Message observer integration ([`McpObserver`] hook for send/receive observability)

---

## Documentation

- [ ] Publish v2 documentation to the docs site — `docs-site/` currently documents
      v1 only (`.docs-major` = `1`, sole snapshot `versioned_docs/version-1.x`).
      Update the guides, API reference, migration docs, and examples for
      2026-07-28.

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

---

## Dependency Updates

See [docs/dependency-policy.md](docs/dependency-policy.md).
