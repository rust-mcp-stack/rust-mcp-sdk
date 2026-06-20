use actix_web::web::PayloadConfig;
use actix_web::{web, Scope};
use rust_mcp_sdk::mcp_http::{McpAppState, McpHttpHandler, McpMountOptions};
use std::sync::Arc;

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
    opts: &McpMountOptions,
) -> Scope {
    let sse_message_endpoint = opts.sse_messages_endpoint.clone();

    let scope = web::scope("")
        .app_data(web::Data::new(state))
        .app_data(web::Data::from(http_handler.clone()))
        .app_data(web::Data::new(crate::routes::sse::SseMessageEndpoint(
            sse_message_endpoint,
        )))
        .app_data(PayloadConfig::new(opts.max_request_body_size))
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
