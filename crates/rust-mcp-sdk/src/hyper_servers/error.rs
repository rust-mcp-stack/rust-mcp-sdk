use std::net::AddrParseError;

use axum::{http::StatusCode, response::IntoResponse};
use thiserror::Error;

use crate::mcp_http::McpHttpError;

#[cfg(feature = "auth")]
use crate::auth::AuthenticationError;

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
    #[error("{0}")]
    HttpError(String),
    #[error("Server start error: {0}")]
    ServerStartError(String),
    #[error("Invalid options: {0}")]
    InvalidServerOptions(String),
    #[error("{0}")]
    SslCertError(String),
    #[error("{0}")]
    TransportError(String),
    #[cfg(feature = "auth")]
    #[error("{0}")]
    AuthenticationError(#[from] AuthenticationError),
}

impl IntoResponse for TransportServerError {
    //consume self and returns a Response
    fn into_response(self) -> axum::response::Response {
        let mut response = StatusCode::INTERNAL_SERVER_ERROR.into_response();
        response.extensions_mut().insert(self);
        response
    }
}

impl From<McpHttpError> for TransportServerError {
    fn from(err: McpHttpError) -> Self {
        match err {
            McpHttpError::SessionIdMissing => TransportServerError::SessionIdMissing,
            McpHttpError::SessionIdInvalid(s) => TransportServerError::SessionIdInvalid(s),
            McpHttpError::StreamIoError(s) => TransportServerError::StreamIoError(s),
            McpHttpError::HttpError(s) => TransportServerError::HttpError(s),
            McpHttpError::TransportError(s) => TransportServerError::TransportError(s),
        }
    }
}

impl From<TransportServerError> for McpHttpError {
    fn from(err: TransportServerError) -> Self {
        match err {
            TransportServerError::SessionIdMissing => McpHttpError::SessionIdMissing,
            TransportServerError::SessionIdInvalid(s) => McpHttpError::SessionIdInvalid(s),
            TransportServerError::StreamIoError(s) => McpHttpError::StreamIoError(s),
            TransportServerError::HttpError(s) => McpHttpError::HttpError(s),
            TransportServerError::TransportError(s) => McpHttpError::TransportError(s),

            #[cfg(feature = "auth")]
            TransportServerError::AuthenticationError(e) => McpHttpError::HttpError(e.to_string()),

            TransportServerError::AddrParseError(e) => McpHttpError::HttpError(e.to_string()),
            TransportServerError::ServerStartError(s) => McpHttpError::HttpError(s),
            TransportServerError::InvalidServerOptions(s) => McpHttpError::HttpError(s),
            TransportServerError::SslCertError(s) => McpHttpError::HttpError(s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp_http::McpHttpResult;

    // McpHttpError to TransportServerError

    #[test]
    fn mcp_to_transport_session_id_missing() {
        let m = McpHttpError::SessionIdMissing;
        let t: TransportServerError = m.into();
        assert!(matches!(t, TransportServerError::SessionIdMissing));
        assert_eq!(format!("{}", t), "'sessionId' query string is missing!");
    }

    #[test]
    fn mcp_to_transport_session_id_invalid() {
        let m = McpHttpError::SessionIdInvalid("s1".into());
        let t: TransportServerError = m.into();
        assert!(matches!(t, TransportServerError::SessionIdInvalid(ref s) if s == "s1"));
        assert_eq!(format!("{}", t), "No session found for the given ID: s1.");
    }

    #[test]
    fn mcp_to_transport_stream_io_error() {
        let m = McpHttpError::StreamIoError("io".into());
        let t: TransportServerError = m.into();
        assert!(matches!(t, TransportServerError::StreamIoError(ref s) if s == "io"));
    }

    #[test]
    fn mcp_to_transport_http_error() {
        let m = McpHttpError::HttpError("fail".into());
        let t: TransportServerError = m.into();
        assert!(matches!(t, TransportServerError::HttpError(ref s) if s == "fail"));
    }

    #[test]
    fn mcp_to_transport_transport_error() {
        let m = McpHttpError::TransportError("tcp".into());
        let t: TransportServerError = m.into();
        assert!(matches!(t, TransportServerError::TransportError(ref s) if s == "tcp"));
    }

    // TransportServerError to McpHttpError (common variants)

    #[test]
    fn transport_to_mcp_session_id_missing() {
        let t = TransportServerError::SessionIdMissing;
        let m: McpHttpError = t.into();
        assert!(matches!(m, McpHttpError::SessionIdMissing));
    }

    #[test]
    fn transport_to_mcp_session_id_invalid() {
        let t = TransportServerError::SessionIdInvalid("s2".into());
        let m: McpHttpError = t.into();
        assert!(matches!(m, McpHttpError::SessionIdInvalid(ref s) if s == "s2"));
    }

    #[test]
    fn transport_to_mcp_stream_io_error() {
        let t = TransportServerError::StreamIoError("eof".into());
        let m: McpHttpError = t.into();
        assert!(matches!(m, McpHttpError::StreamIoError(ref s) if s == "eof"));
    }

    #[test]
    fn transport_to_mcp_http_error() {
        let t = TransportServerError::HttpError("gone".into());
        let m: McpHttpError = t.into();
        assert!(matches!(m, McpHttpError::HttpError(ref s) if s == "gone"));
    }

    #[test]
    fn transport_to_mcp_transport_error() {
        let t = TransportServerError::TransportError("tls".into());
        let m: McpHttpError = t.into();
        assert!(matches!(m, McpHttpError::TransportError(ref s) if s == "tls"));
    }

    // TransportServerError to McpHttpError (lossy conversions)

    #[test]
    fn transport_to_mcp_addr_parse_lossy() {
        use std::net::AddrParseError;
        let parse_err: AddrParseError = ":::".parse::<std::net::IpAddr>().unwrap_err();
        let t = TransportServerError::AddrParseError(parse_err);
        let m: McpHttpError = t.into();
        assert!(matches!(m, McpHttpError::HttpError(ref s) if !s.is_empty()));
    }

    #[test]
    fn transport_to_mcp_server_start_lossy() {
        let t = TransportServerError::ServerStartError("port in use".into());
        let m: McpHttpError = t.into();
        assert!(matches!(m, McpHttpError::HttpError(ref s) if s == "port in use"));
    }

    #[test]
    fn transport_to_mcp_invalid_options_lossy() {
        let t = TransportServerError::InvalidServerOptions("bad config".into());
        let m: McpHttpError = t.into();
        assert!(matches!(m, McpHttpError::HttpError(ref s) if s == "bad config"));
    }

    #[test]
    fn transport_to_mcp_ssl_cert_lossy() {
        let t = TransportServerError::SslCertError("cert expired".into());
        let m: McpHttpError = t.into();
        assert!(matches!(m, McpHttpError::HttpError(ref s) if s == "cert expired"));
    }

    #[cfg(feature = "auth")]
    #[test]
    fn transport_to_mcp_authentication_lossy() {
        let auth_err = AuthenticationError::InactiveToken;
        let t = TransportServerError::AuthenticationError(auth_err);
        let m: McpHttpError = t.into();
        assert!(matches!(m, McpHttpError::HttpError(ref s) if s.contains("Inactive")));
    }

    // Round-trip: McpHttpError to TransportServerError to McpHttpError

    #[test]
    fn round_trip_session_id_missing() {
        let m = McpHttpError::SessionIdMissing;
        let t: TransportServerError = m.clone().into();
        let back: McpHttpError = t.into();
        assert_eq!(format!("{}", m), format!("{}", back));
    }

    #[test]
    fn round_trip_session_id_invalid() {
        let m = McpHttpError::SessionIdInvalid("round".into());
        let t: TransportServerError = m.clone().into();
        let back: McpHttpError = t.into();
        assert_eq!(format!("{}", m), format!("{}", back));
    }

    #[test]
    fn round_trip_stream_io_error() {
        let m = McpHttpError::StreamIoError("pipe".into());
        let t: TransportServerError = m.clone().into();
        let back: McpHttpError = t.into();
        assert_eq!(format!("{}", m), format!("{}", back));
    }

    #[test]
    fn round_trip_http_error() {
        let m = McpHttpError::HttpError("round".into());
        let t: TransportServerError = m.clone().into();
        let back: McpHttpError = t.into();
        assert_eq!(format!("{}", m), format!("{}", back));
    }

    #[test]
    fn round_trip_transport_error() {
        let m = McpHttpError::TransportError("round".into());
        let t: TransportServerError = m.clone().into();
        let back: McpHttpError = t.into();
        assert_eq!(format!("{}", m), format!("{}", back));
    }

    //  Round-trip: TransportServerError > McpHttpError > TransportServerError

    #[test]
    fn reverse_round_trip_session_id_missing() {
        let t = TransportServerError::SessionIdMissing;
        let m: McpHttpError = t.clone().into();
        let back: TransportServerError = m.into();
        assert_eq!(format!("{}", t), format!("{}", back));
    }

    #[test]
    fn reverse_round_trip_session_id_invalid() {
        let t = TransportServerError::SessionIdInvalid("rev".into());
        let m: McpHttpError = t.clone().into();
        let back: TransportServerError = m.into();
        assert_eq!(format!("{}", t), format!("{}", back));
    }

    #[test]
    fn reverse_round_trip_stream_io_error() {
        let t = TransportServerError::StreamIoError("rev".into());
        let m: McpHttpError = t.clone().into();
        let back: TransportServerError = m.into();
        assert_eq!(format!("{}", t), format!("{}", back));
    }

    #[test]
    fn reverse_round_trip_http_error() {
        let t = TransportServerError::HttpError("rev".into());
        let m: McpHttpError = t.clone().into();
        let back: TransportServerError = m.into();
        assert_eq!(format!("{}", t), format!("{}", back));
    }

    #[test]
    fn reverse_round_trip_transport_error() {
        let t = TransportServerError::TransportError("rev".into());
        let m: McpHttpError = t.clone().into();
        let back: TransportServerError = m.into();
        assert_eq!(format!("{}", t), format!("{}", back));
    }

    #[test]
    fn transport_result_from_mcp_http_error() {
        let r: McpHttpResult<()> = Err(McpHttpError::SessionIdMissing);
        let t: TransportServerResult<()> = r.map_err(Into::into);
        assert!(matches!(
            t.unwrap_err(),
            TransportServerError::SessionIdMissing
        ));
    }

    #[test]
    fn mcp_http_result_from_transport_error() {
        let r: TransportServerResult<()> = Err(TransportServerError::SessionIdInvalid("x".into()));
        let m: McpHttpResult<()> = r.map_err(Into::into);
        assert!(matches!(
            m.unwrap_err(),
            McpHttpError::SessionIdInvalid(ref s) if s == "x"
        ));
    }
}
