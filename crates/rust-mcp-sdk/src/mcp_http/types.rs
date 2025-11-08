use crate::{
    mcp_http::McpAppState,
    mcp_server::error::{TransportServerError, TransportServerResult},
};
use bytes::Bytes;
use http::{Request, Response};
use http_body_util::{combinators::BoxBody, BodyExt, Full};
use std::{future::Future, pin::Pin, sync::Arc};

pub type GenericBody = BoxBody<Bytes, TransportServerError>;

pub trait GenericBodyExt {
    fn from_string(s: String) -> Self;
}

impl GenericBodyExt for GenericBody {
    fn from_string(s: String) -> Self {
        Full::new(Bytes::from(s))
            .map_err(|err| TransportServerError::HttpError(err.to_string()))
            .boxed()
    }
}

pub type BoxFutureResponse<'req> =
    Pin<Box<dyn Future<Output = TransportServerResult<Response<GenericBody>>> + Send + 'req>>;

// Define a short alias for your handler function type.
/// A handler function that processes an HTTP request and shared state,
/// returning an async response future.
pub type RequestHandlerFn =
    dyn for<'req> Fn(Request<&'req str>, Arc<McpAppState>) -> BoxFutureResponse<'req> + Send + Sync;

/// A shared, reference-counted request handler.
pub type RequestHandler = Arc<RequestHandlerFn>;

// pub type RequestHandler = Arc<
//     dyn for<'req> FnOnce(Request<&'req str>) -> BoxFutureResponse<'req> + Send + Sync
// >;

pub type MiddlewareNext<'req> =
    Arc<dyn Fn(Request<&'req str>, Arc<McpAppState>) -> BoxFutureResponse<'req> + Send + Sync>;
