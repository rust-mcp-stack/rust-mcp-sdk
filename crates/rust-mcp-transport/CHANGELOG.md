# Changelog

## [0.6.2](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-transport-v0.6.1...rust-mcp-transport-v0.6.2) (2025-10-20)


### 🐛 Bug Fixes

* Mcp client stderr handling ([#113](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/113)) ([84a635e](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/84a635ee445a08c08f4858d7663f2a26a0c79751))


### 🚜 Code Refactoring

* Eventstore with better error handling and stability ([#109](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/109)) ([150e3a0](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/150e3a02ba593b2e41b16d2d621e770d292cfa23))

## [0.6.1](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-transport-v0.6.0...rust-mcp-transport-v0.6.1) (2025-10-13)


### 🚀 Features

* **server:** Decouple core logic from HTTP server for improved architecture ([#106](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/106)) ([d10488b](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/d10488bac739bf28b45d636129eb598d4dd87fd2))

## [0.6.0](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-transport-v0.5.0...rust-mcp-transport-v0.6.0) (2025-09-19)


### ⚠ BREAKING CHANGES

* add Streamable HTTP Client , multiple refactoring and improvements ([#98](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/98))
* update ServerHandler and ServerHandlerCore traits ([#96](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/96))

### 🚀 Features

* Add Streamable HTTP Client , multiple refactoring and improvements ([#98](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/98)) ([abb0c36](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/abb0c36126b0a397bc20a1de36c5a5a80924a01e))
* Event store support for resumability ([#101](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/101)) ([08742bb](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/08742bb9636f81ee79eda4edc192b3b8ed4c7287))
* Update ServerHandler and ServerHandlerCore traits ([#96](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/96)) ([a2d6d23](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/a2d6d23ab59fbc34d04526e2606f747f93a8468c))


### 🐛 Bug Fixes

* Correct pending_requests instance ([#94](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/94)) ([9d8c1fb](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/9d8c1fbdf3ddb7c67ce1fb7dcb8e50b8ba2e1202))

## [0.5.1](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-transport-v0.5.0...rust-mcp-transport-v0.5.1) (2025-08-31)


### 🐛 Bug Fixes

* Correct pending_requests instance ([#94](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/94)) ([9d8c1fb](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/9d8c1fbdf3ddb7c67ce1fb7dcb8e50b8ba2e1202))

## [0.5.0](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-transport-v0.4.1...rust-mcp-transport-v0.5.0) (2025-08-19)


### ⚠ BREAKING CHANGES

* improve request ID generation, remove deprecated methods and adding improvements

### 🚀 Features

* Improve request ID generation, remove deprecated methods and adding improvements ([95b91aa](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/95b91aad191e1b8777ca4a02612ab9183e0276d3))

## [0.4.1](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-transport-v0.4.0...rust-mcp-transport-v0.4.1) (2025-08-12)


### 🚀 Features

* Add Streamable HTTP Support to MCP Server ([#76](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/76)) ([1864ce8](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/1864ce85775912ef6062d70cf9a3dcaf18cf7308))

## [0.4.0](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-transport-v0.3.6...rust-mcp-transport-v0.4.0) (2025-07-03)


### ⚠ BREAKING CHANGES

* implement support for the MCP protocol version 2025-06-18 ([#73](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/73))

### 🚀 Features

* Implement support for the MCP protocol version 2025-06-18 ([#73](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/73)) ([6a24f78](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/6a24f782a7314c3adf302e0c24b42d3fcaae8753))


### 🐛 Bug Fixes

* Exclude assets from published packages ([#70](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/70)) ([0b73873](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/0b738738939708449d9037abbc563d9470f55f8a))

## [0.3.6](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-transport-v0.3.5...rust-mcp-transport-v0.3.6) (2025-06-20)


### 🐛 Bug Fixes

* Sync reqwest dependencies in rust-mcp-transport ([f76468e](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/f76468eec7efb37f530a7c32f1de561b7bf2e21f))

## [0.3.5](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-transport-v0.3.4...rust-mcp-transport-v0.3.5) (2025-06-17)


### 🚀 Features

* Improve schema version configuration using Cargo features ([#51](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/51)) ([836e765](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/836e765613bcaf61b71bb8e0ffe7c9e2877feb22))

## [0.3.4](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-transport-v0.3.3...rust-mcp-transport-v0.3.4) (2025-05-30)


### 🚀 Features

* Multi protocol version - phase 1 ([#49](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/49)) ([4c4daf0](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/4c4daf0b1dce2554ecb7ed4fb723a1c3dd07e541))

## [0.3.3](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-transport-v0.3.2...rust-mcp-transport-v0.3.3) (2025-05-28)


### 🐛 Bug Fixes

* Ensure custom headers are included in initial SSE connection to remote MCP Server ([#46](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/46)) ([166939e](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/166939ee47218675e3883cb86209cd95aa19957e))

## [0.3.2](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-transport-v0.3.1...rust-mcp-transport-v0.3.2) (2025-05-25)


### 🚀 Features

* Improve build process and dependencies ([#38](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/38)) ([e88c4f1](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/e88c4f1c4c4743b13aedbf2a3d65fedb12942555))

## [0.3.1](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-transport-v0.3.0...rust-mcp-transport-v0.3.1) (2025-05-24)


### 🐛 Bug Fixes

* Ensure server resilience against malformed client requests ([95aed88](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/95aed8873e234b4d7d2e0027d2c43be0b0dcc1ab))

## [0.3.0](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-transport-v0.2.3...rust-mcp-transport-v0.3.0) (2025-05-23)


### ⚠ BREAKING CHANGES

* update crates to default to the latest MCP schema version. ([#35](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/35))

### 🚀 Features

* Update crates to default to the latest MCP schema version. ([#35](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/35)) ([6cbc3da](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/6cbc3da9d99d62723643000de74c4bd9e48fa4b4))

## [0.2.3](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-transport-v0.2.2...rust-mcp-transport-v0.2.3) (2025-05-20)


### 🐛 Bug Fixes

* Crate packaging issue caused by stray Cargo.toml ([5475b1b](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/5475b1bb31b5ec2c211bd49f940be38db17d0d65))

## [0.2.2](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-transport-v0.2.1...rust-mcp-transport-v0.2.2) (2025-05-20)


### 🚀 Features

* Add sse transport support ([#32](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/32)) ([1cf1877](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/1cf187757810e142e97216476ca73ecba020c320))

## [0.2.1](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-transport-v0.2.0...rust-mcp-transport-v0.2.1) (2025-04-26)


### 🚀 Features

* Upgrade to rust-mcp-schema v0.4.0 ([#21](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/21)) ([819d113](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/819d1135b469e4aa8e857c81e25c81c331084fb1))


### 🐛 Bug Fixes

* Capture launch errors in client-runtime ([#19](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/19)) ([c0d05ab](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/c0d05ab73b1ac7edc7c410f2f14f0b86d4343c1d))

## [0.2.0](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-transport-v0.1.2...rust-mcp-transport-v0.2.0) (2025-04-16)


### ⚠ BREAKING CHANGES

* naming & less constrained dependencies ([#8](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/8))

### 🚜 Code Refactoring

* Naming & less constrained dependencies ([#8](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/8)) ([2aa469b](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/2aa469b1f7f53f6cda23141c961467ece738047e))

## [0.1.2](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-transport-v0.1.1...rust-mcp-transport-v0.1.2) (2025-04-05)


### 🚀 Features

* Update to latest version of rust-mcp-schema ([#9](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/9)) ([05f4729](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/05f47296e7ef5eff93c5c4e7370a2d1c055328b5))

## [0.1.1](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-transport-v0.1.0...rust-mcp-transport-v0.1.1) (2025-03-29)


### Bug Fixes

* Update crate readme links and docs ([#2](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/2)) ([4f8a5b7](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/4f8a5b74559b97bf9e7229c120c383caf7f53a36))

## [0.1.0](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-transport-v0.1.0...rust-mcp-transport-v0.1.0) (2025-03-29)


### Features

* Initial release v0.1.0 ([4c08beb](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/4c08beb73b102c77e65b724b284008071b7f5ef4))

## [0.1.7](https://github.com/hashemix/rust-mcp-sdk/compare/rust-mcp-transport-v0.1.6...rust-mcp-transport-v0.1.7) (2025-03-24)


### Bug Fixes

* Them all ([2f4990f](https://github.com/hashemix/rust-mcp-sdk/commit/2f4990fbeb9ef5e5b40a7ccb31e9583e318a36ad))

## [0.1.6](https://github.com/hashemix/rust-mcp-sdk/compare/rust-mcp-transport-v0.1.5...rust-mcp-transport-v0.1.6) (2025-03-24)


### Features

* Initial release ([6f6c8ce](https://github.com/hashemix/rust-mcp-sdk/commit/6f6c8cec8fe1277fc39f4ddce6f17b36129bedee))


### Bug Fixes

* Transport ([cab2272](https://github.com/hashemix/rust-mcp-sdk/commit/cab22725fdd2f618020edd4be9b39862d30f2676))
* Transport change ([8eac3ae](https://github.com/hashemix/rust-mcp-sdk/commit/8eac3aeafbcf5f88b81c758fdb0da980a00fa934))

## [0.1.5](https://github.com/hashemix/rust-mcp-sdk/compare/rust-mcp-transport-v0.1.4...rust-mcp-transport-v0.1.5) (2025-03-24)


### Bug Fixes

* Transport change ([8eac3ae](https://github.com/hashemix/rust-mcp-sdk/commit/8eac3aeafbcf5f88b81c758fdb0da980a00fa934))

## [0.1.4](https://github.com/hashemix/rust-mcp-sdk/compare/rust-mcp-transport-v0.1.3...rust-mcp-transport-v0.1.4) (2025-03-24)


### Features

* Initial release ([6f6c8ce](https://github.com/hashemix/rust-mcp-sdk/commit/6f6c8cec8fe1277fc39f4ddce6f17b36129bedee))

## [0.1.3](https://github.com/hashemix/rust-mcp-sdk/compare/v0.1.2...v0.1.3) (2025-03-24)


### Features

* Initial release ([6f6c8ce](https://github.com/hashemix/rust-mcp-sdk/commit/6f6c8cec8fe1277fc39f4ddce6f17b36129bedee))

## [0.1.2](https://github.com/hashemix/rust-mcp-sdk/compare/v0.1.1...v0.1.2) (2025-03-24)


### Features

* Initial release ([6f6c8ce](https://github.com/hashemix/rust-mcp-sdk/commit/6f6c8cec8fe1277fc39f4ddce6f17b36129bedee))

## [0.1.1](https://github.com/hashemix/rust-mcp-sdk/compare/transport-v0.1.0...transport-v0.1.1) (2025-03-24)


### Features

* Initial release ([6f6c8ce](https://github.com/hashemix/rust-mcp-sdk/commit/6f6c8cec8fe1277fc39f4ddce6f17b36129bedee))
