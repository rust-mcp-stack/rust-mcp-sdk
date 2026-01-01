use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rust_mcp_macros::{mcp_resource, mcp_resource_template};
use rust_mcp_schema::{CompleteResultCompletion, TextResourceContents};
use rust_mcp_sdk::schema::{BlobResourceContents, McpMetaEx, ReadResourceResult, RpcError};
use serde_json::Map;

/// A static resource provider for a simple plain-text example.
///
/// This resource demonstrates how to expose readable text content via the MCP readresource request.
/// It serves a famous movie quote as a self-contained, static text resource.
#[mcp_resource(
    name = "Resource 1",
    description = "A plain text resource",
    title = "A plain text resource",
    mime_type = "text/plain",
    uri="test://static/resource/1",
    icons = [
        ( src = "https://raw.githubusercontent.com/rust-mcp-stack/rust-mcp-sdk/main/assets/text-resource.png",
          sizes = ["128x128"],
          mime_type = "image/png" )
    ]
)]
pub struct PlainTextResource {}
impl PlainTextResource {
    pub async fn get_resource() -> std::result::Result<ReadResourceResult, RpcError> {
        Ok(ReadResourceResult {
            contents: vec![TextResourceContents::new(
                "Resource 1: I'm gonna need a bigger boat",
                Self::resource_uri(),
            )
            .with_mime_type("text/plain")
            .into()],
            meta: None,
        })
    }
}

/// A static resource provider for a binary/blob example demonstrating base64-encoded content.
///
/// This resource serves as a simple, self-contained example of how to expose arbitrary binary data
/// (or base64-encoded text) via the MCP ReadResource request.
///
/// The embedded payload is the base64 encoding of the string:
/// `"Resource 2: I'm gonna need a bigger boat"`
#[mcp_resource(
    name = "Resource 2",
    description = "A blob resource",
    title = "A blob resource",
    mime_type = "application/octet-stream",
    uri="test://static/resource/2",
    icons = [
        ( src = "https://raw.githubusercontent.com/rust-mcp-stack/rust-mcp-sdk/main/assets/blob-resource.png",
          sizes = ["128x128"],
          mime_type = "image/png" )
    ]
)]
pub struct BlobTextResource {}
impl BlobTextResource {
    pub async fn get_resource() -> std::result::Result<ReadResourceResult, RpcError> {
        Ok(ReadResourceResult {
            contents: vec![BlobResourceContents::new(
                "UmVzb3VyY2UgMjogSSdtIGdvbm5hIG5lZWQgYSBiaWdnZXIgYm9hdA==",
                Self::resource_uri(),
            )
            .with_mime_type("application/octet-stream")
            .into()],
            meta: None,
        })
    }
}

/// This struct enables MCP servers to expose official Pokémon sprites as resources
///
/// ### URI Scheme
/// - Custom protocol: `pokemon://<id>`
/// - Example: `pokemon://25` → Pikachu sprite
/// - The `<id>` is the Pokémon's National Pokédex number (1–1010+).
///
/// The sprite is fetched from the public [PokeAPI sprites repository](https://github.com/PokeAPI/sprites).
#[mcp_resource_template(
    name = "pokemon",
    description = "Official front-facing sprite of a Pokémon from the PokéAPI sprites",
    title = "Pokémon Sprite",
    mime_type = "image/png",
    uri_template = "pokemon://{pokemon-id}",
    audience = ["user", "assistant"],
    meta = r#"{
        "source": "PokeAPI",
        "repository": "https://github.com/PokeAPI/sprites",
        "license": "CC-BY-4.0",
        "attribution": "Data from PokeAPI - https://pokeapi.co/"
    }"#,
    icons = [
        ( src = "https://raw.githubusercontent.com/rust-mcp-stack/rust-mcp-sdk/main/assets/pokemon-icon.png",
          sizes = ["96x96"],
          mime_type = "image/png" )
    ]
)]
pub struct PokemonImageResource {}
impl PokemonImageResource {
    pub fn matches_url(uri: &str) -> bool {
        uri.starts_with("pokemon://")
    }

    //
    // Demonstration-only completion logic; not performance optimized.
    //
    pub fn completion(pokemon_id: &str) -> CompleteResultCompletion {
        let max_result = 100;

        // All Pokémon IDs as `String`s ranging from `"1"` to `"1050"`,
        let pokemon_ids: Vec<String> = (1..=1050).map(|i| i.to_string()).collect();

        let matched_ids = pokemon_ids
            .iter()
            .filter(|id| id.starts_with(pokemon_id))
            .cloned()
            .collect::<Vec<_>>();

        let has_more = matched_ids.len() > max_result;

        CompleteResultCompletion {
            has_more: has_more.then_some(true),
            total: (!matched_ids.is_empty()).then_some(matched_ids.len() as i64),
            values: matched_ids.into_iter().take(max_result).collect::<Vec<_>>(),
        }
    }

    pub async fn get_resource(uri: &str) -> std::result::Result<ReadResourceResult, RpcError> {
        let id = uri.replace("pokemon://", "");

        let pokemon_uri = format!(
            "https://raw.githubusercontent.com/PokeAPI/sprites/master/sprites/pokemon/{}.png",
            id.trim()
        );

        let client = reqwest::Client::builder()
            .user_agent("rust-mcp-sdk")
            .build()
            .map_err(|e| {
                RpcError::internal_error().with_message(format!("Failed to build HTTP client: {e}"))
            })?;

        let response = client.get(&pokemon_uri).send().await.map_err(|e| {
            RpcError::invalid_request().with_message(format!("Failed to fetch image: {e}"))
        })?;

        if !response.status().is_success() {
            return Err(RpcError::invalid_params().with_message(format!(
                "Image not found (HTTP {}): {pokemon_uri}",
                response.status()
            )));
        }

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();

        // Extract MIME type
        let mime_type = content_type
            .split(';')
            .next()
            .unwrap_or("application/octet-stream")
            .trim()
            .to_string();

        let bytes = response.bytes().await.map_err(|e| {
            RpcError::internal_error().with_message(format!("Failed to read image bytes: {e}"))
        })?;

        let base64_content = BASE64.encode(&bytes);

        let meta = Map::new()
            .add("source", "PokeAPI")
            .add("repository", "https://github.com/PokeAPI/sprites")
            .add("attribution", "Data from PokeAPI - https://pokeapi.co/");

        Ok(ReadResourceResult {
            contents: vec![BlobResourceContents::new(base64_content, pokemon_uri)
                .with_mime_type(mime_type)
                .with_meta(meta.clone())
                .into()],
            meta: Some(meta.clone()),
        })
    }
}
