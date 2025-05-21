use crate::{
    hyper_servers::{
        app_state::AppState,
        error::{TransportServerError, TransportServerResult},
    },
    utils::remove_query_and_hash,
};
use axum::{
    extract::{Query, State},
    response::IntoResponse,
    routing::post,
    Router,
};
use std::{collections::HashMap, sync::Arc};
use tokio::io::AsyncWriteExt;

pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new().route(
        remove_query_and_hash(&state.sse_message_endpoint).as_str(),
        post(handle_messages),
    )
}

pub async fn handle_messages(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
    message: String,
) -> TransportServerResult<impl IntoResponse> {
    let session_id = params
        .get("sessionId")
        .ok_or(TransportServerError::SessionIdMissing)?;

    let transmit =
        state
            .session_store
            .get(session_id)
            .await
            .ok_or(TransportServerError::SessionIdInvalid(
                session_id.to_string(),
            ))?;
    let mut transmit = transmit.lock().await;

    transmit
        .write_all(format!("{message}\n").as_bytes())
        .await
        .map_err(|err| TransportServerError::StreamIoError(err.to_string()))?;

    transmit
        .flush()
        .await
        .map_err(|err| TransportServerError::StreamIoError(err.to_string()))?;

    Ok(axum::http::StatusCode::OK)
}
