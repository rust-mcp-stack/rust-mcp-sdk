use rust_mcp_macros::{mcp_resource, mcp_resource_template};
use rust_mcp_sdk::schema::{
    BlobResourceContents, ReadResourceResult, RpcError, TextResourceContents,
};

const IMAGE_BASE64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";

// ---------------
// 1. test://static-text
// ---------------
#[mcp_resource(
    name = "Static Text Resource",
    description = "A static text resource for conformance testing.",
    mime_type = "text/plain",
    uri = "test://static-text"
)]
pub struct StaticTextResource;

impl StaticTextResource {
    pub async fn get_resource() -> Result<ReadResourceResult, RpcError> {
        let uri = Self::resource_uri().to_string();
        Ok(ReadResourceResult {
            contents: vec![TextResourceContents::new(
                "This is the content of the static text resource.",
                &uri,
            )
            .with_mime_type("text/plain")
            .into()],
            meta: None,
        })
    }
}

// ---------------
// 2. test://static-binary
// ---------------
#[mcp_resource(
    name = "Static Binary Resource",
    description = "A static binary (image) resource for conformance testing.",
    mime_type = "image/png",
    uri = "test://static-binary"
)]
pub struct StaticBinaryResource;

impl StaticBinaryResource {
    pub async fn get_resource() -> Result<ReadResourceResult, RpcError> {
        let uri = Self::resource_uri().to_string();
        Ok(ReadResourceResult {
            contents: vec![BlobResourceContents::new(IMAGE_BASE64, &uri)
                .with_mime_type("image/png")
                .into()],
            meta: None,
        })
    }
}

// ---------------
// 3. test://template/{id}/data
// ---------------
#[mcp_resource_template(
    name = "Template Resource",
    description = "A resource template with parameter substitution for conformance testing.",
    mime_type = "application/json",
    uri_template = "test://template/{id}/data"
)]
pub struct TemplateDataResource;

impl TemplateDataResource {
    pub fn matches_url(uri: &str) -> bool {
        uri.starts_with("test://template/") && uri.ends_with("/data")
    }

    pub async fn get_resource(uri: &str) -> Result<ReadResourceResult, RpcError> {
        let id = uri
            .strip_prefix("test://template/")
            .and_then(|s| s.strip_suffix("/data"))
            .unwrap_or("unknown");

        let content = serde_json::json!({
            "id": id,
            "templateTest": true,
            "data": format!("Data for ID: {}", id)
        });

        Ok(ReadResourceResult {
            contents: vec![
                TextResourceContents::new(content.to_string(), uri.to_string())
                    .with_mime_type("application/json")
                    .into(),
            ],
            meta: None,
        })
    }
}

// ---------------
// 4. test://embedded-resource
// ---------------
#[mcp_resource(
    name = "Embedded Resource",
    description = "A resource used for embedded content tests.",
    mime_type = "text/plain",
    uri = "test://embedded-resource"
)]
pub struct EmbeddedTestResource;

impl EmbeddedTestResource {
    pub async fn get_resource() -> Result<ReadResourceResult, RpcError> {
        let uri = Self::resource_uri().to_string();
        Ok(ReadResourceResult {
            contents: vec![TextResourceContents::new(
                "This is an embedded resource content.",
                &uri,
            )
            .with_mime_type("text/plain")
            .into()],
            meta: None,
        })
    }
}

// ---------------
// 5. test://watched-resource
// ---------------
#[mcp_resource(
    name = "Watched Resource",
    description = "A subscribable resource that updates periodically for conformance testing.",
    mime_type = "application/json",
    uri = "test://watched-resource"
)]
pub struct WatchedResource;

impl WatchedResource {
    pub async fn get_resource() -> Result<ReadResourceResult, RpcError> {
        let uri = Self::resource_uri().to_string();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs().to_string())
            .unwrap_or_else(|_| "unknown".into());
        let content = serde_json::json!({
            "watched": true,
            "timestamp": timestamp
        });

        Ok(ReadResourceResult {
            contents: vec![
                TextResourceContents::new(content.to_string(), uri.to_string())
                    .with_mime_type("application/json")
                    .into(),
            ],
            meta: None,
        })
    }
}
