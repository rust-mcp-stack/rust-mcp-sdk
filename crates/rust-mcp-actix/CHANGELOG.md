# Changelog

## [2.0.0](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-actix-v1.0.2...rust-mcp-actix-v2.0.0) (2026-08-27)


### ⚠ BREAKING CHANGES

* migrate to MCP 2026-07-28 (stateless) ([#240](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/240))

### 🚀 Features

* Migrate to MCP 2026-07-28 (stateless) ([#240](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/240)) ([809faea](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/809faeae4ea4e636653026133f5836987377ab82))

## [1.0.2](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-actix-v1.0.1...rust-mcp-actix-v1.0.2) (2026-08-26)


### 📚 Documentation

* Add versioned Docusaurus site, GitHub Pages deploy & release automation ([#239](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/239)) ([006b7bf](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/006b7bfec21cd41e006ed734505768703343bbb1))

## [1.0.1](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-actix-v1.0.0...rust-mcp-actix-v1.0.1) (2026-07-26)

## [1.0.0](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-actix-v0.1.4...rust-mcp-actix-v1.0.0) (2026-07-25)


### 🚀 Features

* V1.0.0 release preparation and audit ([#219](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/219)) ([b609d94](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/b609d9491b8308ed106fb1cf8070b57b1edef9b6))


### 🚜 Code Refactoring

* Replace rustls-pemfile with rustls-pki-types PemObject API ([449394a](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/449394ae7e445a2046365d1d24414f0bb590f7bd))

## [0.1.4](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-actix-v0.1.3...rust-mcp-actix-v0.1.4) (2026-06-24)


### 🐛 Bug Fixes

* Update crate metadata ([7619890](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/7619890583143b716100ac1cf4736549b5c0c96e))

## [0.1.3](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-actix-v0.1.2...rust-mcp-actix-v0.1.3) (2026-06-24)


### 🐛 Bug Fixes

* Initial publish ([0d815bd](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/0d815bdf7cda2fd8dc6fe7626a317fe6d2a024bf))

## [0.1.2](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-actix-v0.1.1...rust-mcp-actix-v0.1.2) (2026-06-24)


### 🐛 Bug Fixes

* Missig version in mcp-actix and mc-axum ([c33d2e6](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/c33d2e6800ac79af60b0affaa5d5cbe192b76d74))

## [0.1.1](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-actix-v0.1.0...rust-mcp-actix-v0.1.1) (2026-06-24)


### 🚀 Features

* Enforce request body size limits with shared McpMountOptions ([#163](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/163)) ([8dd749a](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/8dd749aa9aa5cac9aef8d7a931bb16e947e01a42))
* Initial release v0.1.0 ([4c08beb](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/4c08beb73b102c77e65b724b284008071b7f5ef4))
* **session:** Make session store injectable with bounded default ([#167](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/167)) ([af601b5](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/af601b56e236fe0e10bb5659e7f6ed6b90917d51))


### 🐛 Bug Fixes

* **http:** Auto-derive DNS rebinding allowed_hosts from bind address ([#165](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/165)) ([55d8a03](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/55d8a03764921329e0e81f0d406452ca3313bffd))


### ⚡ Performance Improvements

* Extract POST body as Bytes ([#166](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/166)) ([352a5fd](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/352a5fd890378181dda1f893e91ef4a5e16d6b63))


### 📚 Documentation

* Documentation audit, examples upgrade, and test stabilization ([#174](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/174)) ([667e522](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/667e522f407291b34ec16b8fbd4068586207f2b6))
* Prepare v0.10.0 release ,upgrading.md, and migration guide ([#175](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/175)) ([4086a08](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/4086a08742b1d46ff8abb17ce3796727de8e6ec3))
