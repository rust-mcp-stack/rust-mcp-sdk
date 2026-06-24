//! Non-auth conformance scenarios. Each module exposes a single async
//! `run` function with the same signature `async fn run(server_url: &str)`,
//! invoked by the binary's `main()` based on the scenario name.

pub mod elicitation;
pub mod initialize;
pub mod sse_retry;
pub mod tools_call;
