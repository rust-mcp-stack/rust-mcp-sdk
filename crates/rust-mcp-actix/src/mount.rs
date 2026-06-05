use actix_web::{web, Scope};
use rust_mcp_sdk::mcp_http::{McpAppState, McpHttpHandler};
use std::sync::Arc;

/// Mount configuration for BYO-server scenarios with Actix.
///
/// Use with [`mcp_scope()`] to mount MCP endpoints on an existing Actix application.
pub struct ActixMountOptions {
    pub streamable_http_endpoint: String,
    pub sse_endpoint: String,
    pub sse_messages_endpoint: String,
    pub health_endpoint: Option<String>,
}

impl Clone for ActixMountOptions {
    fn clone(&self) -> Self {
        Self {
            streamable_http_endpoint: self.streamable_http_endpoint.clone(),
            sse_endpoint: self.sse_endpoint.clone(),
            sse_messages_endpoint: self.sse_messages_endpoint.clone(),
            health_endpoint: self.health_endpoint.clone(),
        }
    }
}

impl Default for ActixMountOptions {
    fn default() -> Self {
        Self {
            streamable_http_endpoint: "/mcp".to_string(),
            sse_endpoint: "/sse".to_string(),
            sse_messages_endpoint: "/messages".to_string(),
            health_endpoint: None,
        }
    }
}

/// Builds an Actix [`Scope`] with all MCP endpoint routes mounted.
///
/// This is the **BYO-server** mount function:
///
/// ```ignore
/// HttpServer::new(move || {
///     App::new()
///         .service(web::scope("/api").route("", web::get().to(my_handler)))
///         .service(rust_mcp_actix::mcp_scope(state.clone(), handler.clone(), &opts))
/// })
/// .bind("127.0.0.1:8080")?
/// .run()
/// .await?;
/// ```
pub fn mcp_scope(
    state: Arc<McpAppState>,
    http_handler: Arc<McpHttpHandler>,
    opts: &ActixMountOptions,
) -> Scope {
    let sse_message_endpoint = opts.sse_messages_endpoint.clone();

    let scope = web::scope("")
        .app_data(web::Data::new(state))
        .app_data(web::Data::from(http_handler.clone()))
        .app_data(web::Data::new(crate::routes::sse::SseMessageEndpoint(
            sse_message_endpoint,
        )))
        .service(
            web::resource(&opts.streamable_http_endpoint)
                .route(web::get().to(crate::routes::streamable_http::handle_streamable_http_get))
                .route(web::post().to(crate::routes::streamable_http::handle_streamable_http_post))
                .route(
                    web::delete().to(crate::routes::streamable_http::handle_streamable_http_delete),
                ),
        )
        .service(
            web::resource(&opts.sse_endpoint).route(web::get().to(crate::routes::sse::handle_sse)),
        )
        .service(
            web::resource(&opts.sse_messages_endpoint)
                .route(web::post().to(crate::routes::messages::handle_messages)),
        );

    // Mount auth routes
    let scope = if let Some(auth_scope) = crate::routes::auth::auth_routes(http_handler) {
        scope.service(auth_scope)
    } else {
        scope
    };

    // Mount health check if enabled
    let scope = if let Some(ref endpoint) = opts.health_endpoint {
        scope.service(
            web::resource(endpoint)
                .route(web::get().to(crate::routes::health::handle_health_check)),
        )
    } else {
        scope
    };

    // Fallback for unmatched routes
    scope.default_service(web::route().to(crate::routes::fallback::not_found))
}
