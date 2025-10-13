# Changelog

## [0.7.1](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-sdk-v0.7.0...rust-mcp-sdk-v0.7.1) (2025-10-13)


### 🚀 Features

* Add server_supports_completion method ([#104](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/104)) ([6268726](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/62687262a30cce0928435c153b6016d56e85b8ee))
* **server:** Decouple core logic from HTTP server for improved architecture ([#106](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/106)) ([d10488b](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/d10488bac739bf28b45d636129eb598d4dd87fd2))


### ⚡ Performance Improvements

* Remove unnecesarry mutex in the session store ([ea5d580](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/ea5d58013ac051f2bbe7e9f5b3a20a3220e66c9b))


### 🚜 Code Refactoring

* Expose Store Traits and add ToMcpServerHandler for Improved Framework Flexibility ([#107](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/107)) ([5bf54d6](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/5bf54d6d442d6cb854242697fa50c29bca0b8483))

## [0.7.0](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-sdk-v0.6.3...rust-mcp-sdk-v0.7.0) (2025-09-19)


### ⚠ BREAKING CHANGES

* add Streamable HTTP Client , multiple refactoring and improvements ([#98](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/98))
* update ServerHandler and ServerHandlerCore traits ([#96](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/96))

### 🚀 Features

* Add elicitation macros and add elicit_input() method ([#99](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/99)) ([3ab5fe7](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/3ab5fe73aaa10de2b5b23caee357ac15b37c845f))
* Add Streamable HTTP Client , multiple refactoring and improvements ([#98](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/98)) ([abb0c36](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/abb0c36126b0a397bc20a1de36c5a5a80924a01e))
* Add tls-no-provider feature ([#97](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/97)) ([5dacceb](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/5dacceb0c2d18b8334744a13d438c6916bb7244c))
* Event store support for resumability ([#101](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/101)) ([08742bb](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/08742bb9636f81ee79eda4edc192b3b8ed4c7287))
* Update ServerHandler and ServerHandlerCore traits ([#96](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/96)) ([a2d6d23](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/a2d6d23ab59fbc34d04526e2606f747f93a8468c))

## [0.6.3](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-sdk-v0.6.2...rust-mcp-sdk-v0.6.3) (2025-08-31)

## [0.6.2](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-sdk-v0.6.1...rust-mcp-sdk-v0.6.2) (2025-08-30)


### 🐛 Bug Fixes

* Tool-box macro panic on invalid requests ([#92](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/92)) ([54cc8ed](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/54cc8edb55c41455dd9211f296560e7a792a7b9c))

## [0.6.1](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-sdk-v0.6.0...rust-mcp-sdk-v0.6.1) (2025-08-28)


### 🐛 Bug Fixes

* Session ID access in handlers and add helper for listing active ([#90](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/90)) ([f2f0afb](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/f2f0afb542f6ff036a28cf01e102b27ce940665b))

## [0.6.0](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-sdk-v0.5.3...rust-mcp-sdk-v0.6.0) (2025-08-19)


### ⚠ BREAKING CHANGES

* improve request ID generation, remove deprecated methods and adding improvements

### 🚀 Features

* Improve request ID generation, remove deprecated methods and adding improvements ([95b91aa](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/95b91aad191e1b8777ca4a02612ab9183e0276d3))

## [0.5.3](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-sdk-v0.5.2...rust-mcp-sdk-v0.5.3) (2025-08-19)


### 🐛 Bug Fixes

* Handle missing client details and abort keep-alive task on drop ([#83](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/83)) ([308b1db](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/308b1dbd1744ff06046902303d8bcd6c3a92ffbe))

## [0.5.2](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-sdk-v0.5.1...rust-mcp-sdk-v0.5.2) (2025-08-16)


### 🚀 Features

* Integrate list root and client info into hyper runtime ([36dfa4c](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/36dfa4cdc821e958ffe78b909ed28f5577d113c8))


### 🐛 Bug Fixes

* Abort keep-alive task when transport is removed ([#82](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/82)) ([1ca8e49](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/1ca8e49860e990c3562623e75dd723b0d1dc8256))
* Ensure server-initiated requests include a valid request_id ([#80](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/80)) ([5f9a966](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/5f9a966bb523bf61daefcff209199bc774fa5ed6))

## [0.5.1](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-sdk-v0.5.0...rust-mcp-sdk-v0.5.1) (2025-08-12)


### 🚀 Features

* Add Streamable HTTP Support to MCP Server ([#76](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/76)) ([1864ce8](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/1864ce85775912ef6062d70cf9a3dcaf18cf7308))
* Update examples and docs for streamable http ([#77](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/77)) ([e714482](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/e714482e65d8b4ff6e0942b15609f920d78235d9))


### 📚 Documentation

* Update readme ([31d5d67](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/31d5d672bf340292a6f54961bab09afbe468539e))
* Update readme ([470a51a](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/470a51a8bf8b0ed78b8dfb171e43eb847d8a0666))

## [0.5.0](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-sdk-v0.4.7...rust-mcp-sdk-v0.5.0) (2025-07-03)


### ⚠ BREAKING CHANGES

* implement support for the MCP protocol version 2025-06-18 ([#73](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/73))

### 🚀 Features

* Implement support for the MCP protocol version 2025-06-18 ([#73](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/73)) ([6a24f78](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/6a24f782a7314c3adf302e0c24b42d3fcaae8753))


### 🐛 Bug Fixes

* Address issue with improper server start failure handling ([#72](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/72)) ([fc4d664](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/fc4d6646e050ab84ab15fcd8a2f95109df4af256))
* Exclude assets from published packages ([#70](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/70)) ([0b73873](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/0b738738939708449d9037abbc563d9470f55f8a))

## [0.4.7](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-sdk-v0.4.6...rust-mcp-sdk-v0.4.7) (2025-06-29)


### 🚀 Features

* Make hyper optional ([#63](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/63)) ([8dd95a2](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/8dd95a2a112d6c661ddc3deede2dd606b4ff743b))

## [0.4.6](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-sdk-v0.4.5...rust-mcp-sdk-v0.4.6) (2025-06-23)


### 🐛 Bug Fixes

* Allow optional trailing commas in tool_box macro ([#58](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/58)) ([ce0cc4f](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/ce0cc4f564a95d964f28e4f52e8d4fa5d4ae9e60))

## [0.4.5](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-sdk-v0.4.4...rust-mcp-sdk-v0.4.5) (2025-06-20)

## [0.4.4](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-sdk-v0.4.3...rust-mcp-sdk-v0.4.4) (2025-06-20)


### 🚀 Features

* Enable rustls support in reqwest ([d6c6293](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/d6c6293c4ac66fadafee7385b80e0f4cd002e7e4))

## [0.4.3](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-sdk-v0.4.2...rust-mcp-sdk-v0.4.3) (2025-06-17)


### 🚀 Features

* Improve schema version configuration using Cargo features ([#51](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/51)) ([836e765](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/836e765613bcaf61b71bb8e0ffe7c9e2877feb22))

## [0.4.2](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-sdk-v0.4.1...rust-mcp-sdk-v0.4.2) (2025-05-30)


### 🚀 Features

* Multi protocol version - phase 1 ([#49](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/49)) ([4c4daf0](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/4c4daf0b1dce2554ecb7ed4fb723a1c3dd07e541))

## [0.4.1](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-sdk-v0.4.0...rust-mcp-sdk-v0.4.1) (2025-05-28)

## [0.4.0](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-sdk-v0.3.3...rust-mcp-sdk-v0.4.0) (2025-05-28)


### ⚠ BREAKING CHANGES

* make rust-mcp-sdk the sole dependency ([#43](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/43))

### 🚀 Features

* Make rust-mcp-sdk the sole dependency ([#43](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/43)) ([d1973ca](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/d1973ca037c1c6367261bb48a9a4ec89c3a448ac))

## [0.3.3](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-sdk-v0.3.2...rust-mcp-sdk-v0.3.3) (2025-05-25)


### 🐛 Bug Fixes

* Prevent termination caused by client using older mcp schema versions ([#40](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/40)) ([084d9d3](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/084d9d36c37c135256873bffd46d2ca03a1fb330))

## [0.3.2](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-sdk-v0.3.1...rust-mcp-sdk-v0.3.2) (2025-05-25)


### 🚀 Features

* Improve build process and dependencies ([#38](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/38)) ([e88c4f1](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/e88c4f1c4c4743b13aedbf2a3d65fedb12942555))

## [0.3.1](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-sdk-v0.3.0...rust-mcp-sdk-v0.3.1) (2025-05-24)

## [0.3.0](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-sdk-v0.2.6...rust-mcp-sdk-v0.3.0) (2025-05-23)


### ⚠ BREAKING CHANGES

* update crates to default to the latest MCP schema version. ([#35](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/35))

### 🚀 Features

* Update crates to default to the latest MCP schema version. ([#35](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/35)) ([6cbc3da](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/6cbc3da9d99d62723643000de74c4bd9e48fa4b4))

## [0.2.6](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-sdk-v0.2.5...rust-mcp-sdk-v0.2.6) (2025-05-20)

## [0.2.5](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-sdk-v0.2.4...rust-mcp-sdk-v0.2.5) (2025-05-20)


### 🚀 Features

* Add sse transport support ([#32](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/32)) ([1cf1877](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/1cf187757810e142e97216476ca73ecba020c320))

## [0.2.4](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-sdk-v0.2.3...rust-mcp-sdk-v0.2.4) (2025-05-01)


### 📚 Documentation

* Update documentation ([#26](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/26)) ([4cf3cb1](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/4cf3cb1db8effe10632adb32e7a350cdcdedd69b))

## [0.2.3](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-sdk-v0.2.2...rust-mcp-sdk-v0.2.3) (2025-05-01)


### 🐛 Bug Fixes

* Remove unnecessary error wrapper ([#24](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/24)) ([b919fba](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/b919fbabd143125df35486f9fd0d5af0c156a2d8))

## [0.2.2](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-sdk-v0.2.1...rust-mcp-sdk-v0.2.2) (2025-04-26)


### 🚀 Features

* Upgrade to rust-mcp-schema v0.4.0 ([#21](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/21)) ([819d113](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/819d1135b469e4aa8e857c81e25c81c331084fb1))


### 🐛 Bug Fixes

* Capture launch errors in client-runtime ([#19](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/19)) ([c0d05ab](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/c0d05ab73b1ac7edc7c410f2f14f0b86d4343c1d))

## [0.2.1](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-sdk-v0.2.0...rust-mcp-sdk-v0.2.1) (2025-04-20)


### 🚀 Features

* Introduce Cargo features to isolate client and server code ([#18](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/18)) ([1fa9a6f](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/1fa9a6f60ec2ece34b68e49855c13489a0889d48))


### 📚 Documentation

* Add projects list to readme ([#16](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/16)) ([deee010](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/deee010c84228c00a7f4426d560f7ceb5d2d274f))

## [0.2.0](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-sdk-v0.1.3...rust-mcp-sdk-v0.2.0) (2025-04-16)


### ⚠ BREAKING CHANGES

* naming & less constrained dependencies ([#8](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/8))

### 🚜 Code Refactoring

* Naming & less constrained dependencies ([#8](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/8)) ([2aa469b](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/2aa469b1f7f53f6cda23141c961467ece738047e))

## [0.1.3](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-sdk-v0.1.2...rust-mcp-sdk-v0.1.3) (2025-04-05)


### 🚀 Features

* Update to latest version of rust-mcp-schema ([#9](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/9)) ([05f4729](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/05f47296e7ef5eff93c5c4e7370a2d1c055328b5))

## [0.1.2](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-sdk-v0.1.1...rust-mcp-sdk-v0.1.2) (2025-03-30)


### 🚀 Features

* Re-export transport and macros for seamless user experience ([#4](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/4)) ([ff9e3af](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/ff9e3af0e43a6e915f968445b1fbdb54a5069a8b))


### 📚 Documentation

* Add step by step guide to the project to help getting started quickly ([#6](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/6)) ([571f36a](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/571f36a452164bea24065eddb8d8591f665f2d80))

## [0.1.1](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-sdk-v0.1.0...rust-mcp-sdk-v0.1.1) (2025-03-29)


### Bug Fixes

* Update crate readme links and docs ([#2](https://github.com/rust-mcp-stack/rust-mcp-sdk/issues/2)) ([4f8a5b7](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/4f8a5b74559b97bf9e7229c120c383caf7f53a36))

## [0.1.0](https://github.com/rust-mcp-stack/rust-mcp-sdk/compare/rust-mcp-sdk-v0.1.0...rust-mcp-sdk-v0.1.0) (2025-03-29)


### Features

* Initial release v0.1.0 ([4c08beb](https://github.com/rust-mcp-stack/rust-mcp-sdk/commit/4c08beb73b102c77e65b724b284008071b7f5ef4))

## [0.1.7](https://github.com/hashemix/rust-mcp-sdk/compare/rust-mcp-sdk-v0.1.6...rust-mcp-sdk-v0.1.7) (2025-03-24)


### Bug Fixes

* Them all ([2f4990f](https://github.com/hashemix/rust-mcp-sdk/commit/2f4990fbeb9ef5e5b40a7ccb31e9583e318a36ad))

## [0.1.6](https://github.com/hashemix/rust-mcp-sdk/compare/rust-mcp-sdk-v0.1.5...rust-mcp-sdk-v0.1.6) (2025-03-24)


### Bug Fixes

* Sdk ([75fded9](https://github.com/hashemix/rust-mcp-sdk/commit/75fded976925cf24c25cdffacab7f31e468c0f08))

## [0.1.5](https://github.com/hashemix/rust-mcp-sdk/compare/rust-mcp-sdk-v0.1.4...rust-mcp-sdk-v0.1.5) (2025-03-24)


### Bug Fixes

* Sdk change ([5a4d636](https://github.com/hashemix/rust-mcp-sdk/commit/5a4d63675bf71bf26443453d9f00bf91b49d29d1))

## [0.1.4](https://github.com/hashemix/rust-mcp-sdk/compare/rust-mcp-sdk-v0.1.3...rust-mcp-sdk-v0.1.4) (2025-03-24)


### Features

* Initial release ([6f6c8ce](https://github.com/hashemix/rust-mcp-sdk/commit/6f6c8cec8fe1277fc39f4ddce6f17b36129bedee))

## [0.1.3](https://github.com/hashemix/rust-mcp-sdk/compare/v0.1.2...v0.1.3) (2025-03-24)


### Features

* Initial release ([6f6c8ce](https://github.com/hashemix/rust-mcp-sdk/commit/6f6c8cec8fe1277fc39f4ddce6f17b36129bedee))

## [0.1.2](https://github.com/hashemix/rust-mcp-sdk/compare/v0.1.1...v0.1.2) (2025-03-24)


### Features

* Initial release ([6f6c8ce](https://github.com/hashemix/rust-mcp-sdk/commit/6f6c8cec8fe1277fc39f4ddce6f17b36129bedee))

## [0.1.1](https://github.com/hashemix/rust-mcp-sdk/compare/Rust MCP SDK-v0.1.0...Rust MCP SDK-v0.1.1) (2025-03-24)


### Features

* Initial release ([6f6c8ce](https://github.com/hashemix/rust-mcp-sdk/commit/6f6c8cec8fe1277fc39f4ddce6f17b36129bedee))
