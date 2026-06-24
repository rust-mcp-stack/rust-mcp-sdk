//! Entry point for the `conformance-client` binary.
//!
//! The binary is a thin dispatcher: it reads the test scenario from the
//! `MCP_CONFORMANCE_SCENARIO` environment variable (set by the official
//! conformance harness), parses any per-scenario context from
//! `MCP_CONFORMANCE_CONTEXT`, and delegates to the matching scenario
//! module under [`client`].
//!
//! Each scenario module is independently testable and focused on one MCP
//! protocol feature — see `client/scenarios/` and `client/auth/`.

mod client;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let server_url = args.last().expect("Server URL required as last argument");

    let scenario = std::env::var("MCP_CONFORMANCE_SCENARIO").unwrap_or_default();
    let context: serde_json::Value = serde_json::from_str(
        &std::env::var("MCP_CONFORMANCE_CONTEXT").unwrap_or_else(|_| "{}".into()),
    )
    .unwrap_or_default();

    match scenario.as_str() {
        "initialize" | "" => client::scenarios::initialize::run(server_url).await,
        "tools_call" => client::scenarios::tools_call::run(server_url).await,
        s if s.starts_with("elicitation") => client::scenarios::elicitation::run(server_url).await,
        s if s.starts_with("sse") => client::scenarios::sse_retry::run(server_url).await,
        s if s.starts_with("auth/") => client::auth::run(server_url, &context).await,
        other => {
            eprintln!("Unknown or unimplemented scenario: {}", other);
            std::process::exit(1);
        }
    }
}
