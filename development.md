# Development

This document outlines the process for compiling this crate's source code on your local machine.

## Prerequisites

Ensure you have the following installed:

- The latest stable version of **Rust**
- [`cargo-nextest`](https://crates.io/crates/cargo-nextest) for running tests
- [`cargo-make`](https://crates.io/crates/cargo-make/0.3.54) for running tasks like tests

## Setting Up the Development Environment

1- Clone the repository:

```sh
git clone https://github.com/rust-mcp-stack/rust-mcp-sdk
cd rust-mcp-sdk
```

2- Install dependencies: The Rust project uses Cargo for dependency management. To install dependencies, run:

```sh
cargo build
```

## Running Examples

Example projects can be found in the [/examples](/examples) folder of the repository.
Build and run instructions are available in their respective README.md files.

You can run examples by passing the example project name to Cargo using the `-p` argument, like this:

```sh
cargo run -p simple-mcp-client
```

You can build the examples in a similar way. The following command builds the project and generates the binary at `target/release/hello-world-mcp-server`:

```sh

cargo build -p hello-world-mcp-server --release
```

## Code Formatting

We follow the default Rust formatting style enforced by `rustfmt`. To format your code, run:

```sh
cargo fmt
```

Additionally, we use **Clippy** for linting Rust code. You can check for linting issues by running:

```sh
cargo make clippy
```

Please ensure your code is formatted and free of Clippy warnings before submitting any changes.

## Testing

We use [`cargo-nextest`](https://crates.io/crates/cargo-nextest) to run our test suite.

### Running Tests

To run the tests, use:

```sh
cargo make test
```
