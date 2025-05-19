use axum::{
    http::{StatusCode, Uri},
    Router,
};

pub fn routes() -> Router {
    Router::new().fallback(not_found)
}

pub async fn not_found(uri: Uri) -> (StatusCode, String) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Server Error!\r\n uri: {}", uri),
    )
}
