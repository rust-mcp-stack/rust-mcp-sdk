use axum::{
    http::{StatusCode, Uri},
    Router,
};

pub fn routes() -> Router {
    Router::new().fallback(not_found)
}

pub async fn not_found(uri: Uri) -> (StatusCode, String) {
    (
        StatusCode::NOT_FOUND,
        format!("The requested uri does not exist:\r\nuri: {uri}"),
    )
}
