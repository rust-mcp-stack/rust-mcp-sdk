//! `discover` scenario — connect, verify the server returns its
//! capabilities via `server/discover`, disconnect.

use crate::client::transport;
use rust_mcp_sdk::schema::RequestParams;

pub async fn run(server_url: &str) {
    let client = transport::connect(server_url)
        .await
        .expect("Failed to connect");
    let result = client
        .request_discover(RequestParams::default())
        .await
        .expect("Failed to discover server");
    assert!(
        result.capabilities.tools.is_some()
            || result.capabilities.prompts.is_some()
            || result.capabilities.resources.is_some(),
        "Server should advertise at least one capability"
    );
    client.shut_down().await.ok();
}
