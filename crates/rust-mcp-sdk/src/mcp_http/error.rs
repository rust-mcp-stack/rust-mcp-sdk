use thiserror::Error;

pub type McpHttpResult<T> = core::result::Result<T, McpHttpError>;

#[derive(Debug, Clone, Error)]
pub enum McpHttpError {
    #[error("Stream IO Error: {0}.")]
    StreamIoError(String),

    #[error("{0}")]
    HttpError(String),

    #[error("{0}")]
    TransportError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_stream_io_error() {
        let err = McpHttpError::StreamIoError("broken pipe".into());
        assert_eq!(format!("{}", err), "Stream IO Error: broken pipe.");
    }

    #[test]
    fn display_http_error() {
        let err = McpHttpError::HttpError("bad request".into());
        assert_eq!(format!("{}", err), "bad request");
    }

    #[test]
    fn display_transport_error() {
        let err = McpHttpError::TransportError("timeout".into());
        assert_eq!(format!("{}", err), "timeout");
    }

    #[test]
    fn clone_preserves_value() {
        let err = McpHttpError::StreamIoError("xyz".into());
        let cloned = err.clone();
        assert_eq!(format!("{}", err), format!("{}", cloned));
    }

    #[test]
    fn debug_format_includes_variant() {
        let err = McpHttpError::StreamIoError("test".into());
        let debug = format!("{:?}", err);
        assert!(debug.contains("StreamIoError"));
    }

    #[test]
    fn clone_unit_variant() {
        let err = McpHttpError::HttpError("oops".into());
        let cloned = err.clone();
        assert_eq!(format!("{:?}", err), format!("{:?}", cloned));
    }

    #[test]
    fn http_result_ok() {
        let result: McpHttpResult<i32> = Ok(42);
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn http_result_err() {
        let result: McpHttpResult<i32> = Err(McpHttpError::HttpError("fail".into()));
        assert!(result.is_err());
        assert_eq!(format!("{}", result.unwrap_err()), "fail");
    }
}
