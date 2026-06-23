# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.9.x   | Yes       |
| < 0.9   | No        |

## Reporting a Vulnerability

If you discover a security vulnerability in `rust-mcp-sdk`, please report it responsibly.

**Do NOT open a public GitHub issue for security vulnerabilities.**

### How to Report

Use [GitHub Security Advisories](https://github.com/rust-mcp-stack/rust-mcp-sdk/security/advisories/new)

### What to Include

- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

### Response Timeline

| Severity | Acknowledgment | Resolution |
|----------|---------------|------------|
| Critical (P0) | 1 business day | 7 days |
| High (P1) | 2 business days | 14 days |
| Medium (P2) | 5 business days | 30 days |
| Low (P3) | 10 business days | Next release |

### Process

1. We acknowledge receipt within the timeline above
2. We investigate and confirm the vulnerability
3. We develop a fix in a private branch
4. We release the fix and publish a security advisory
5. We credit the reporter (unless they prefer anonymity)

### Scope

This policy covers:
- `rust-mcp-sdk` — MCP protocol implementation
- `rust-mcp-transport` — Transport layer (stdio, SSE, streamable HTTP)
- `rust-mcp-axum` — Axum HTTP integration
- `rust-mcp-actix` — Actix HTTP integration
- `rust-mcp-macros` — Proc-macro code generation
- `rust-mcp-schema` — Protocol schema types

### Known Security Considerations

- **DNS Rebinding**: Localhost MCP servers must validate Host/Origin headers. The SDK provides `DnsRebindingOptions` middleware for this purpose.
- **Transport Security**: Stdio transport is local-only. HTTP transports should use TLS in production.
- **Input Validation**: Tool input schemas are validated by the SDK. Servers should additionally validate business logic.
