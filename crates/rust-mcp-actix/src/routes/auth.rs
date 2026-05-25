use actix_web::{web, HttpRequest, Scope};
use rust_mcp_sdk::mcp_http::{McpAppState, McpHttpHandler};
use std::sync::Arc;

pub fn auth_routes(http_handler: Arc<McpHttpHandler>) -> Option<Scope> {
    let endpoints: Vec<String> = http_handler
        .oauth_endppoints()
        .unwrap_or_default()
        .into_iter()
        .cloned()
        .collect();

    if endpoints.is_empty() {
        return None;
    }

    let mut scope = web::scope("");
    for endpoint in endpoints {
        let handler = http_handler.clone();
        scope = scope.route(
            &endpoint,
            web::route().to(
                move |req: HttpRequest, state: web::Data<Arc<McpAppState>>, payload: String| {
                    let handler = handler.clone();
                    async move {
                        let request = crate::bridge::from_actix_request(&req, Some(&payload));
                        match handler
                            .handle_auth_requests(request, state.get_ref().clone())
                            .await
                        {
                            Ok(res) => crate::bridge::to_actix_response(res).await,
                            Err(err) => crate::bridge::to_actix_error(err),
                        }
                    }
                },
            ),
        );
    }
    Some(scope)
}
