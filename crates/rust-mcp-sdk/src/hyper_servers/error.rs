use std::net::AddrParseError;

use axum::{http::StatusCode, response::IntoResponse};
use thiserror::Error;

pub type TransportServerResult<T> = core::result::Result<T, TransportServerError>;

#[derive(Debug, Error, Clone)]
pub enum TransportServerError {
    #[error("'sessionId' query string is missing!")]
    SessionIdMissing,
    #[error("No session found for the given ID: {0}.")]
    SessionIdInvalid(String),
    #[error("Stream IO Error: {0}.")]
    StreamIoError(String),
    #[error("{0}")]
    AddrParseError(#[from] AddrParseError),
    #[error("Server start error: {0}")]
    ServerStartError(String),
    #[error("Invalid options: {0}")]
    InvalidServerOptions(String),
    #[error("{0}")]
    SslCertError(String),
}

impl IntoResponse for TransportServerError {
    //consume self and returns a Response
    fn into_response(self) -> axum::response::Response {
        let mut response = StatusCode::INTERNAL_SERVER_ERROR.into_response();
        response.extensions_mut().insert(self);
        response
    }
}
