//! This module provides a simple observer that logs incoming/outgoing messages to a remote server via HTTP POST.
//! The logs can be monitored at https://app.beeceptor.com/console/rustmcp

use rust_mcp_sdk::{
    schema::{ClientMessage, McpMessage, RpcMessage, ServerMessage},
    McpObserver,
};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(Debug)]
pub struct SimpleClientObserver {
    sender: mpsc::Sender<LogEntry>,
}

#[derive(Debug)]
struct LogEntry {
    kind: String,
    message_type: String,
    message_id: Option<String>,
    method: Option<String>,
}

impl SimpleClientObserver {
    const BASE_URL: &str = "https://rustmcp.free.beeceptor.com/log";

    /// Create a new observer and spawn the background worker
    pub fn new() -> Arc<Self> {
        let (tx, mut rx) = mpsc::channel::<LogEntry>(1000);

        // Spawn background task to send logs
        tokio::spawn(async move {
            let client = reqwest::Client::new();
            while let Some(entry) = rx.recv().await {
                let _ = client
                    .post(Self::BASE_URL)
                    .json(&json!({
                        "kind": entry.kind,
                        "type": entry.message_type,
                        "id": entry.message_id,
                        "method": entry.method
                    }))
                    .send()
                    .await;
            }
        });

        Arc::new(Self { sender: tx })
    }

    /// Send a log entry asynchronously (non-blocking)
    pub fn send_log(
        &self,
        kind: String,
        message_type: String,
        id: Option<String>,
        method: Option<&str>,
    ) {
        let entry = LogEntry {
            kind,
            message_type,
            message_id: id.map(|s| s.to_string()),
            method: method.map(|s| s.to_string()),
        };
        let _ = self.sender.try_send(entry); // Non-blocking
    }
}

impl McpObserver<ServerMessage, ClientMessage> for SimpleClientObserver {
    fn on_receive(&self, message: &ServerMessage) {
        self.send_log(
            "ServerMessage".into(),
            message.message_type().to_string(),
            message.request_id().map(|s| s.to_string()),
            message.method(),
        );
    }

    fn on_send(&self, message: &ClientMessage) {
        self.send_log(
            "ClientMessage".into(),
            message.message_type().to_string(),
            message.request_id().map(|s| s.to_string()),
            message.method(),
        );
    }
}
