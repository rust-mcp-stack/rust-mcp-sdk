# Changelog

## [0.3.1](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-extra-v0.3.0...rust-mcp-extra-v0.3.1) (2026-06-24)

## [0.3.0](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-extra-v0.2.4...rust-mcp-extra-v0.3.0) (2026-06-24)


### ⚠ BREAKING CHANGES

* extract Axum to standalone rust-mcp-axum crate ([#146](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/146))
* extract Axum to standalone rust-mcp-axum crate

### 🚀 Features

* Add allowlist override for JWKS algorithms ([ee680a8](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/ee680a8e3ffd4cc5e3c742c619b63c1cba063e43))
* Extract Axum to standalone rust-mcp-axum crate ([0bd6cf6](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/0bd6cf6721b25c1066c702d2bdf752143ad2ecf3))
* Extract Axum to standalone rust-mcp-axum crate ([#146](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/146)) ([ddc5600](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/ddc56001cd561aef0eccadd0c3bb788c176575ff))


### 🐛 Bug Fixes

* **auth:** Pin JWT validation algorithms to an allowlist ([#148](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/148)) ([c1ee180](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/c1ee1808145fb29ef95b273fdb30bb7139e959eb))
* **auth:** Validate token audience by default ([#149](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/149)) ([1f714bf](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/1f714bf728caefcc1f4c07d853ce90b2622456a1))
* Update getting started document ([#162](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/162)) ([3e22cfe](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/3e22cfee318a104f5c3a0e28a8e0b04410612c32))


### ⚡ Performance Improvements

* **auth:** Reuse a shared reqwest::Client with timeouts ([#172](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/172)) ([c644426](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/c64442644076d5dbcd9437245aecff5c36b4fb32))


### 📚 Documentation

* Documentation audit, examples upgrade, and test stabilization ([#174](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/174)) ([667e522](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/667e522f407291b34ec16b8fbd4068586207f2b6))


### 🚜 Code Refactoring

* Switch mcp http to framework-agnostic McpHttpError  ([#144](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/144)) ([e0c44c0](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/e0c44c0e4f8aaed9bfc59b9274d11c346646a635))
* Switch mcp_http layer to framework-agnostic McpHttpError ([afaf4b1](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/afaf4b1ebfe070f565a1857ed707da678b9d16ae))

## [0.2.4](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-extra-v0.2.3...rust-mcp-extra-v0.2.4) (2026-03-13)

## [0.2.3](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-extra-v0.2.2...rust-mcp-extra-v0.2.3) (2026-02-01)

## [0.2.2](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-extra-v0.2.1...rust-mcp-extra-v0.2.2) (2026-01-18)

## [0.2.1](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-extra-v0.2.0...rust-mcp-extra-v0.2.1) (2026-01-01)

## [0.2.0](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-extra-v0.1.4...rust-mcp-extra-v0.2.0) (2026-01-01)


### ⚠ BREAKING CHANGES

* update to MCP Protocol 2025-11-25, new mcp_icon macro and various improvements ([#120](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/120))

### 🚀 Features

* Update to MCP Protocol 2025-11-25, new mcp_icon macro and various improvements ([#120](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/120)) ([e70f8b7](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/e70f8b7e9d4ef028e66d4cd1bf5cd4c96d81adf9))

## [0.1.4](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-extra-v0.1.3...rust-mcp-extra-v0.1.4) (2025-11-23)


### 🚀 Features

* Add authentication flow support to MCP servers ([#119](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/119)) ([fe467d3](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/fe467d3661a60b6bb1f9d5b53697c1a94dc77c12))

## [0.1.3](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-extra-v0.1.2...rust-mcp-extra-v0.1.3) (2025-11-08)


### 🚀 Features

* Refactor and improve middleware pipeline ([#114](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/114)) ([cc45f1c](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/cc45f1c2e6321ef740dda87d229aa51213a06808))

## [0.1.2](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-extra-v0.1.1...rust-mcp-extra-v0.1.2) (2025-10-20)


### 🚀 Features

* Add middleware support to mcp_http_handler ([#112](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/112)) ([18b1e6f](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/18b1e6f3e9671bfffa4bd59f64dc12fc2e44d818))

## [0.1.1](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-extra-v0.1.0...rust-mcp-extra-v0.1.1) (2025-10-13)


### 🚀 Features

* Initial release v0.1.0 ([4c08beb](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/4c08beb73b102c77e65b724b284008071b7f5ef4))
* Introduce `rust-mcp-extra` crate for extended id, session, and event store support ([#108](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/108)) ([5fddd3c](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/5fddd3cee12d622c19c23a67d4f381475d914031))
