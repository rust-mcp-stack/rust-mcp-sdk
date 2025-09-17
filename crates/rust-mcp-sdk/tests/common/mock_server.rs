use axum::{
    body::Body,
    extract::Request,
    http::{header::CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue, Method, StatusCode},
    response::{
        sse::{Event, KeepAlive},
        IntoResponse, Response, Sse,
    },
    routing::any,
    Router,
};
use core::fmt;
use futures::stream;
use std::collections::VecDeque;
use std::{future::Future, net::SocketAddr, pin::Pin};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::net::TcpListener;

pub struct SseEvent {
    /// The optional event type (e.g., "message").
    pub event: Option<String>,
    /// The optional data payload of the event, stored as bytes.
    pub data: Option<String>,
    /// The optional event ID for reconnection or tracking purposes.
    pub id: Option<String>,
}

impl ToString for SseEvent {
    fn to_string(&self) -> String {
        let mut s = String::new();

        if let Some(id) = &self.id {
            s.push_str("id: ");
            s.push_str(id);
            s.push('\n');
        }

        if let Some(event) = &self.event {
            s.push_str("event: ");
            s.push_str(event);
            s.push('\n');
        }

        if let Some(data) = &self.data {
            // Convert bytes to string safely, fallback if invalid UTF-8
            for line in data.lines() {
                s.push_str("data: ");
                s.push_str(line);
                s.push('\n');
            }
        }

        s.push('\n'); // End of event
        s
    }
}

impl fmt::Debug for SseEvent {
    /// Formats the `SseEvent` for debugging, converting the `data` field to a UTF-8 string
    /// (with lossy conversion if invalid UTF-8 is encountered).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let data_str = self.data.as_ref();

        f.debug_struct("SseEvent")
            .field("event", &self.event)
            .field("data", &data_str)
            .field("id", &self.id)
            .finish()
    }
}

// RequestRecord stores the history of incoming requests
#[derive(Clone, Debug)]
pub struct RequestRecord {
    pub method: Method,
    pub path: String,
    pub headers: HeaderMap,
    pub body: String,
}

#[derive(Clone, Debug)]
pub struct ResponseRecord {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: String,
}

// pub type BoxedStream =
//     Pin<Box<dyn futures::Stream<Item = Result<Event, std::convert::Infallible>> + Send>>;
// pub type BoxedSseResponse = Sse<BoxedStream>;

// pub type AsyncResponseFn =
//     Box<dyn Fn() -> Pin<Box<dyn Future<Output = BoxedSseResponse> + Send>> + Send + Sync>;

type AsyncResponseFn =
    Box<dyn Fn() -> Pin<Box<dyn Future<Output = Response> + Send>> + Send + Sync>;

// Mock defines a single mock response configuration
// #[derive(Clone)]
pub struct Mock {
    method: Method,
    path: String,
    response: String,
    response_func: Option<AsyncResponseFn>,
    header_map: HeaderMap,
    matcher: Option<Arc<dyn Fn(&str, &HeaderMap) -> bool + Send + Sync>>,
    remaining_calls: Option<Arc<Mutex<usize>>>,
    status: StatusCode,
}

// MockBuilder is a factory for creating Mock instances
pub struct MockBuilder {
    method: Method,
    path: String,
    response: String,
    header_map: HeaderMap,
    response_func: Option<AsyncResponseFn>,
    matcher: Option<Arc<dyn Fn(&str, &HeaderMap) -> bool + Send + Sync>>,
    remaining_calls: Option<Arc<Mutex<usize>>>,
    status: StatusCode,
}

impl MockBuilder {
    fn new(method: Method, path: String, response: String, header_map: HeaderMap) -> Self {
        Self {
            method,
            path,
            response,
            response_func: None,
            header_map,
            matcher: None,
            status: StatusCode::OK,
            remaining_calls: None, // Default to unlimited calls
        }
    }

    fn new_with_func(
        method: Method,
        path: String,
        response_func: AsyncResponseFn,
        header_map: HeaderMap,
    ) -> Self {
        Self {
            method,
            path,
            response: String::new(),
            response_func: Some(response_func),
            header_map,
            matcher: None,
            status: StatusCode::OK,
            remaining_calls: None, // Default to unlimited calls
        }
    }

    pub fn new_breakable_sse(
        method: Method,
        path: String,
        repeating_message: SseEvent,
        interval: Duration,
        repeat: usize,
    ) -> Self {
        let message = Arc::new(repeating_message);
        let interval = interval;
        let max_repeats = repeat;

        let response_fn: AsyncResponseFn = Box::new({
            let message = Arc::clone(&message);
            move || {
                let message = Arc::clone(&message);

                Box::pin(async move {
                    // Construct SSE stream with 10 static messages using unfold
                    let message_stream = stream::unfold(0, move |count| {
                        let message = Arc::clone(&message);

                        async move {
                            if count >= max_repeats {
                                return Some((
                                    Err(std::io::Error::other("Message limit reached")),
                                    count,
                                ));
                            }
                            tokio::time::sleep(interval).await;

                            Some((
                                Ok(Event::default()
                                    .data(message.data.clone().unwrap_or("".into()))
                                    .id(message.id.clone().unwrap_or(format!("msg-id_{count}")))
                                    .event(message.event.clone().unwrap_or("message".into()))),
                                count + 1,
                            ))
                        }
                    });

                    let sse_stream = Sse::new(message_stream)
                        .keep_alive(KeepAlive::new().interval(Duration::from_secs(10)));

                    sse_stream.into_response()
                })
            }
        });

        let mut header_map = HeaderMap::new();
        header_map.insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
        Self::new_with_func(method, path, response_fn, header_map)
    }

    pub fn with_matcher<F>(mut self, matcher: F) -> Self
    where
        F: Fn(&str, &HeaderMap) -> bool + Send + Sync + 'static,
    {
        self.matcher = Some(Arc::new(matcher));
        self
    }

    pub fn add_header(mut self, key: HeaderName, val: HeaderValue) -> Self {
        self.header_map.insert(key, val);
        self
    }

    pub fn without_matcher(mut self) -> Self {
        self.matcher = None;
        self
    }

    pub fn expect(mut self, num_calls: usize) -> Self {
        self.remaining_calls = Some(Arc::new(Mutex::new(num_calls)));
        self
    }

    pub fn unlimited_calls(mut self) -> Self {
        self.remaining_calls = None;
        self
    }

    pub fn with_status(mut self, status: StatusCode) -> Self {
        self.status = status;
        self
    }

    pub fn build(self) -> Mock {
        Mock {
            method: self.method,
            path: self.path,
            response: self.response,
            header_map: self.header_map,
            matcher: self.matcher,
            remaining_calls: self.remaining_calls,
            status: self.status,
            response_func: self.response_func,
        }
    }

    // add_string with text/plain
    pub fn new_text(method: Method, path: String, response: impl Into<String>) -> Self {
        let mut header_map = HeaderMap::new();
        header_map.insert(CONTENT_TYPE, HeaderValue::from_static("text/plain"));

        Self::new(method, path, response.into(), header_map)
    }

    /**
     MockBuilder::new_response(
        Method::GET,
        "/mcp".to_string(),
        Box::new(|| {
            // tokio::time::sleep(Duration::from_secs(1)).await;
            let json_response = Json(json!({
                "status": "ok",
                "data": [1, 2, 3]
            }))
            .into_response();
            Box::pin(async move { json_response })
        }),
    )
    .build(),
    */
    pub fn new_response(method: Method, path: String, response_func: AsyncResponseFn) -> Self {
        Self::new_with_func(method, path, response_func, HeaderMap::new())
    }

    // new_json with application/json
    pub fn new_json(method: Method, path: String, response: impl Into<String>) -> Self {
        let mut header_map = HeaderMap::new();
        header_map.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        Self::new(method, path, response.into(), header_map)
    }

    // new_sse with text/event-stream
    pub fn new_sse(method: Method, path: String, response: impl Into<String>) -> Self {
        let response = format!(r#"data: {}{}"#, response.into(), '\n');

        let mut header_map = HeaderMap::new();
        header_map.insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
        // ensure message ends with a \n\n , if needed
        let cr = if response.ends_with("\n\n") {
            ""
        } else {
            "\n\n"
        };
        Self::new(method, path, format!("{response}{cr}"), header_map)
    }

    // new_raw with application/octet-stream
    pub fn new_raw(method: Method, path: String, response: impl Into<String>) -> Self {
        let mut header_map = HeaderMap::new();
        header_map.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
        Self::new(method, path, response.into(), header_map)
    }
}

// MockServerHandle provides access to the request history after the server starts
pub struct MockServerHandle {
    history: Arc<Mutex<VecDeque<(RequestRecord, ResponseRecord)>>>,
}

impl MockServerHandle {
    pub async fn get_history(&self) -> Vec<(RequestRecord, ResponseRecord)> {
        let history = self.history.lock().unwrap();
        history.iter().cloned().collect()
    }

    pub async fn print(&self) {
        let requests = self.get_history().await;

        let len = requests.len();
        println!("\n>>>  {len} request(s) received <<<");

        for (index, (request, response)) in requests.iter().enumerate() {
            println!(
                "\n--- Request {} of {len} ------------------------------------",
                index + 1
            );
            println!("Method: {}", request.method);
            println!("Path: {}", request.path);
            // println!("Headers: {:#?}", request.headers);
            println!("> headers ");
            for (key, values) in &request.headers {
                println!("{key}: {values:?}");
            }

            println!("\n> Body");
            println!("{}\n", &request.body);

            println!(">>>>> Response <<<<<");
            println!("> status: {}", response.status);
            println!("> headers");
            for (key, values) in &response.headers {
                println!("{key}: {values:?}");
            }
            println!("> Body");
            println!("{}", &response.body);
        }
    }
}

// MockServer is the main struct for configuring and starting the mock server
pub struct SimpleMockServer {
    mocks: Vec<Mock>,
    history: Arc<Mutex<VecDeque<(RequestRecord, ResponseRecord)>>>,
}

impl Default for SimpleMockServer {
    fn default() -> Self {
        Self::new()
    }
}

impl SimpleMockServer {
    pub fn new() -> Self {
        Self {
            mocks: Vec::new(),
            history: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub async fn start_with_mocks(mocks: Vec<Mock>) -> (String, MockServerHandle) {
        let mut server = SimpleMockServer::new();
        server.add_mocks(mocks);
        server.start().await
    }

    // Generic add function
    pub fn add_mock_builder(&mut self, builder: MockBuilder) -> &mut Self {
        self.mocks.push(builder.build());
        self
    }

    pub fn add_mock(&mut self, mock: Mock) -> &mut Self {
        self.mocks.push(mock);
        self
    }

    pub fn add_mocks(&mut self, mock: Vec<Mock>) -> &mut Self {
        mock.into_iter().for_each(|m| self.mocks.push(m));
        self
    }

    pub async fn start(self) -> (String, MockServerHandle) {
        let mocks = Arc::new(self.mocks);
        let history = Arc::clone(&self.history);

        async fn handler(
            mocks: Arc<Vec<Mock>>,
            history: Arc<Mutex<VecDeque<(RequestRecord, ResponseRecord)>>>,
            mut req: Request,
        ) -> impl IntoResponse {
            // Take ownership of the body using std::mem::take
            let body = std::mem::take(req.body_mut());
            let body_bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
            let body_str = String::from_utf8_lossy(&body_bytes).to_string();

            let request_record = RequestRecord {
                method: req.method().clone(),
                path: req.uri().path().to_string(),
                headers: req.headers().clone(),
                body: body_str.clone(),
            };

            for m in mocks.iter() {
                if m.method != *req.method() || m.path != req.uri().path() {
                    continue;
                }

                if let Some(matcher) = &m.matcher {
                    if !(matcher)(&body_str, req.headers()) {
                        continue;
                    }
                }

                if let Some(remaining) = &m.remaining_calls {
                    let mut rem = remaining.lock().unwrap();
                    if *rem == 0 {
                        continue;
                    }
                    *rem -= 1;
                }

                let mut resp = match m.response_func.as_ref() {
                    Some(get_response) => get_response().await.into_response(),
                    None => Response::new(Body::from(m.response.clone())),
                };

                // if let Some(resp_box) = &mut m.response_func.take() {
                //     let response = resp_box.into_response();
                //     // *response.status_mut() = m.status;
                //     // m.response_func = Some(Box::new(response));
                // }

                // let mut resp = m.response_func.as_ref().unwrap().clone().to_owned();
                // let resp = *resp;
                // *resp.into_response().status_mut() = m.status;

                // let mut response = m.response_func.as_ref().unwrap().clone();
                // let mut response = m.response_func.as_ref().unwrap().clone().to_owned();
                // let mut m = *response;
                // *response.status_mut() = m.status;
                // let resp = &*m.response_func.as_ref().unwrap().to_owned().clone().deref();

                // let response = boxed_response.into_response();

                // let mut resp = Response::new(Body::from(m.response.clone()));
                *resp.status_mut() = m.status;
                m.header_map.iter().for_each(|(k, v)| {
                    resp.headers_mut().insert(k, v.clone());
                });

                let response_record = ResponseRecord {
                    status: resp.status(),
                    headers: resp.headers().clone(),
                    body: m.response.clone(),
                };

                {
                    let mut hist = history.lock().unwrap();
                    hist.push_back((request_record, response_record));
                }

                return resp;
            }

            let resp = Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::empty())
                .unwrap();

            let response_record = ResponseRecord {
                status: resp.status(),
                headers: resp.headers().clone(),
                body: "".into(),
            };

            {
                let mut hist = history.lock().unwrap();
                hist.push_back((request_record, response_record));
            }

            resp
        }

        let app = Router::new().route(
            "/{*path}",
            any(move |req: Request| handler(Arc::clone(&mocks), Arc::clone(&history), req)),
        );

        let addr = SocketAddr::from(([127, 0, 0, 1], 0));
        let listener = TcpListener::bind(addr).await.unwrap();
        let local_addr = listener.local_addr().unwrap();
        let url = format!("http://{local_addr}");

        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        (
            url,
            MockServerHandle {
                history: self.history,
            },
        )
    }
}
