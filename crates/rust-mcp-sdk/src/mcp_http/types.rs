use crate::{
    mcp_http::McpAppState,
    mcp_server::error::{TransportServerError, TransportServerResult},
};
use bytes::Bytes;
use futures::future::BoxFuture;
use http::{
    header::{ALLOW, CONTENT_TYPE},
    HeaderMap, HeaderName, HeaderValue, Method, Request, Response, StatusCode,
};
use http_body_util::{combinators::BoxBody, BodyExt, Full};
use serde_json::Value;
use std::sync::Arc;

pub type GenericBody = BoxBody<Bytes, TransportServerError>;

pub trait GenericBodyExt {
    fn from_string(s: String) -> Self;
    fn from_value(value: &Value) -> Self;
    fn empty() -> Self;
    fn build_response(
        status_code: StatusCode,
        payload: String,
        headers: Option<HeaderMap>,
    ) -> http::Response<GenericBody>;
    fn into_response(
        self,
        status_code: StatusCode,
        headers: Option<HeaderMap>,
    ) -> http::Response<GenericBody>;

    fn into_json_response(
        self,
        status_code: StatusCode,
        headers: Option<HeaderMap>,
    ) -> http::Response<GenericBody>;

    fn create_404_response() -> http::Response<GenericBody>;

    fn create_405_response(
        method: &Method,
        allowed_methods: &[Method],
    ) -> http::Response<GenericBody>;
}

impl GenericBodyExt for GenericBody {
    fn from_string(s: String) -> Self {
        Full::new(Bytes::from(s))
            .map_err(|err| TransportServerError::HttpError(err.to_string()))
            .boxed()
    }

    fn from_value(value: &Value) -> Self {
        let bytes = match serde_json::to_vec(value) {
            Ok(vec) => Bytes::from(vec),
            Err(_) => Bytes::from_static(b"{\"error\":\"internal_error\"}"),
        };
        Full::new(bytes)
            .map_err(|err| TransportServerError::HttpError(err.to_string()))
            .boxed()
    }

    fn empty() -> Self {
        Full::new(Bytes::new())
            .map_err(|err| TransportServerError::HttpError(err.to_string()))
            .boxed()
    }

    fn build_response(
        status_code: StatusCode,
        payload: String,
        headers: Option<HeaderMap>,
    ) -> http::Response<GenericBody> {
        let body = Self::from_string(payload);
        body.into_response(status_code, headers)
    }

    fn into_json_response(
        self,
        status_code: StatusCode,
        headers: Option<HeaderMap>,
    ) -> http::Response<GenericBody> {
        let mut headers = headers.unwrap_or_default();
        headers.append(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        self.into_response(status_code, Some(headers))
    }

    fn into_response(
        self,
        status_code: StatusCode,
        headers: Option<HeaderMap>,
    ) -> http::Response<GenericBody> {
        let mut resp = http::Response::new(self);
        *resp.status_mut() = status_code;

        if let Some(mut headers) = headers {
            let mut current_name: Option<HeaderName> = None;
            for (name_opt, value) in headers.drain() {
                if let Some(name) = name_opt {
                    current_name = Some(name);
                }
                let name = current_name.as_ref().unwrap();
                resp.headers_mut().append(name.clone(), value);
            }
        }
        if !resp.headers().contains_key(CONTENT_TYPE) {
            resp.headers_mut()
                .append(CONTENT_TYPE, HeaderValue::from_static("text/plain"));
        }
        resp
    }

    fn create_404_response() -> http::Response<GenericBody> {
        Self::empty().into_response(StatusCode::NOT_FOUND, None)
    }

    fn create_405_response(
        method: &Method,
        allowed_methods: &[Method],
    ) -> http::Response<GenericBody> {
        let allow_header_value = HeaderValue::from_str(
            allowed_methods
                .iter()
                .map(|m| m.as_str())
                .collect::<Vec<_>>()
                .join(", ")
                .as_str(),
        )
        .unwrap_or(HeaderValue::from_static("unknown"));
        let mut response = Self::from_string(format!(
            "The method {method} is not allowed for this endpoint"
        ))
        .into_response(StatusCode::METHOD_NOT_ALLOWED, None);
        response.headers_mut().append(ALLOW, allow_header_value);
        response
    }
}

pub trait RequestExt {
    fn insert<T: Clone + Send + Sync + 'static>(&mut self, val: T);
    fn get<T: Send + Sync + 'static>(&self) -> Option<&T>;
    fn take<T: Send + Sync + 'static>(self) -> (Self, Option<T>)
    where
        Self: Sized;
}

impl RequestExt for http::Request<&str> {
    fn insert<T: Clone + Send + Sync + 'static>(&mut self, val: T) {
        self.extensions_mut().insert(val);
    }

    fn get<T: Send + Sync + 'static>(&self) -> Option<&T> {
        self.extensions().get::<T>()
    }

    fn take<T: Send + Sync + 'static>(mut self) -> (Self, Option<T>) {
        let exts = self.extensions_mut();
        let val = exts.remove::<T>();
        (self, val)
    }
}

pub type BoxFutureResponse<'req> = BoxFuture<'req, TransportServerResult<Response<GenericBody>>>;
// pub type BoxFutureResponse<'req> =
//     Pin<Box<dyn Future<Output = TransportServerResult<Response<GenericBody>>> + Send + 'req>>;

// Handler function type (can only be called once)
pub type RequestHandlerFnOnce =
    dyn for<'req> FnOnce(Request<&'req str>, Arc<McpAppState>) -> BoxFutureResponse<'req> + Send;

// RequestHandler cannot be Arc<...> anymore because FnOnce isn’t clonable
pub type RequestHandler = Box<RequestHandlerFnOnce>;

// Middleware "next" closure type - can only be called once
pub type MiddlewareNext<'req> =
    Box<dyn FnOnce(Request<&'req str>, Arc<McpAppState>) -> BoxFutureResponse<'req> + Send>;
