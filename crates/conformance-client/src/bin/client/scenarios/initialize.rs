//! `initialize` scenario — connect, verify the server returned its info,
//! disconnect.

use crate::client::transport;

pub async fn run(server_url: &str) {
    let client = transport::connect(server_url)
        .await
        .expect("Failed to connect");
    assert!(
        client.server_info().is_some(),
        "Server info should be set after init"
    );
    client.shut_down().await.ok();
}
