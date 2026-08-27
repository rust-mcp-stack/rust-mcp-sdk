//! `subscriptions/listen` scenario — subscribe to server notifications
//! and verify the subscription result is well-formed.

use crate::client::transport;
use rust_mcp_sdk::schema::{
    RequestMetaObject, SubscriptionFilter, SubscriptionsListenRequestParams,
};

#[allow(dead_code)]
pub async fn run(server_url: &str) {
    let client = transport::connect(server_url)
        .await
        .expect("Failed to connect");

    let params = SubscriptionsListenRequestParams {
        notifications: SubscriptionFilter {
            prompts_list_changed: Some(true),
            resource_subscriptions: vec![],
            resources_list_changed: Some(true),
            tools_list_changed: Some(true),
        },
        meta: RequestMetaObject::default(),
    };

    let result = client
        .request_subscriptions_listen(params)
        .await
        .expect("Failed to subscribe");

    assert!(
        result
            .meta
            .io_modelcontextprotocol_subscription_id
            .to_string()
            != "0",
        "subscriptionId should be set to a valid request id"
    );

    client.shut_down().await.ok();
}
