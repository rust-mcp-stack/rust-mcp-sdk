//! Non-auth conformance scenarios. Each module exposes a single async
//! `run` function with the same signature `async fn run(server_url: &str)`,
//! invoked by the binary's `main()` based on the scenario name.

pub mod elicitation;
pub mod http_custom_headers;
pub mod http_invalid_tool_headers;
pub mod http_standard_headers;
pub mod initialize;
pub mod json_schema_ref_no_deref;
pub mod mrtr;
pub mod request_metadata;
pub mod sse_retry;
pub mod subscriptions_listen;
pub mod tools_call;
