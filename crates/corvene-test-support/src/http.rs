//! A local HTTP server standing in for the network in GitHub Desktop's
//! API tests.
//!
//! GitHub Desktop's tests hand the code under test a `Response` built in
//! the test (`new Response(body, { status, statusText, headers })`), or
//! stub `fetch` / `request` to resolve to one. Corvene's clients read their
//! responses from the network, so [`serve`] answers their requests from a
//! server on `127.0.0.1` with that response, [`serve_with`] picks the
//! response per request (GitHub Desktop's stubs that look at the URL) and
//! records what was asked, and [`unreachable_endpoint`] stands for a stub
//! that throws (nothing listens there).
//!
//! The servers run on a thread each until the test binary exits, one
//! request at a time, and close every connection after one response
//! (`Connection: close`). The isolated environment ([`crate::env`]) clears
//! the proxy variables and sets `NO_PROXY` for `127.0.0.1`, `localhost` and
//! `::1`, so requests to them never leave the machine.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;

/// GitHub Desktop's `new Response(body, { status, statusText, headers })`.
/// `Content-Length` and `Connection: close` are always added.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StubResponse {
    pub status: u16,
    /// The reason phrase of the status line (`statusText`).
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl StubResponse {
    /// `new Response(body, { status })`, with the status's usual reason
    /// phrase ([`reason_phrase`]) and no headers.
    pub fn new(status: u16, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            status_text: reason_phrase(status).to_string(),
            headers: Vec::new(),
            body: body.into(),
        }
    }

    /// The same response with another reason phrase (`statusText`).
    pub fn with_status_text(mut self, status_text: impl Into<String>) -> Self {
        self.status_text = status_text.into();
        self
    }

    /// The same response with one more header.
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

/// The usual reason phrase of the statuses GitHub Desktop's tests use; an
/// empty string for any other status.
pub fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        402 => "Payment Required",
        403 => "Forbidden",
        404 => "Not Found",
        422 => "Unprocessable Entity",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "",
    }
}

/// A request a stub server received.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StubRequest {
    /// `GET`, `POST`…
    pub method: String,
    /// The request target as sent: path and query
    /// (`/api/v3/user/emails?per_page=100`).
    pub target: String,
    /// The request head as received: the request line and the header
    /// lines, each ending in `\r\n`, without the blank line.
    pub head: String,
    pub body: Vec<u8>,
}

impl StubRequest {
    /// [`target`](Self::target) without the query.
    pub fn path(&self) -> &str {
        self.target.split('?').next().unwrap_or_default()
    }

    /// The value of the first header called `name` (any case), trimmed.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.head.lines().skip(1).find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.trim().eq_ignore_ascii_case(name).then(|| value.trim())
        })
    }
}

/// A running stub server ([`serve_with`]). Dropping it does not stop the
/// server.
#[derive(Clone, Debug)]
pub struct StubServer {
    url: String,
    requests: Arc<Mutex<Vec<StubRequest>>>,
}

impl StubServer {
    /// The server's base URL, `http://127.0.0.1:<port>` (no trailing `/`).
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Every request answered so far, oldest first. A request is recorded
    /// before its response is sent, so a client that has its response finds
    /// its request here.
    pub fn requests(&self) -> Vec<StubRequest> {
        self.requests
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// The [`head`](StubRequest::head) of every request answered so far.
    pub fn request_heads(&self) -> Vec<String> {
        self.requests().into_iter().map(|r| r.head).collect()
    }
}

/// Answer every request with `response` until the test binary exits;
/// returns the server's base URL (`http://127.0.0.1:<port>`), to be used as
/// the endpoint of the code under test.
///
/// # Panics
///
/// When no local port can be bound.
pub fn serve(response: StubResponse) -> String {
    serve_with(move |_| response.clone()).url
}

/// Answer each request with what `router` returns for it (GitHub Desktop's
/// stubbed `fetch` that looks at the URL; route on
/// [`StubRequest::path`]) until the test binary exits, recording the
/// requests ([`StubServer::requests`]).
///
/// # Panics
///
/// When no local port can be bound.
pub fn serve_with(router: impl Fn(&StubRequest) -> StubResponse + Send + 'static) -> StubServer {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a local port for a stub server");
    let port = listener
        .local_addr()
        .expect("the stub server's address")
        .port();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let recorded = requests.clone();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { return };
            let Some(request) = read_request(&stream) else {
                continue;
            };
            recorded
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(request.clone());
            write_response(stream, &router(&request));
        }
    });
    StubServer {
        url: format!("http://127.0.0.1:{port}"),
        requests,
    }
}

/// A base URL nothing listens on: every request to it fails before a
/// response arrives (GitHub Desktop's stubbed `request` that throws).
///
/// # Panics
///
/// When no local port can be bound.
pub fn unreachable_endpoint() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a local port");
    let port = listener.local_addr().expect("the local address").port();
    drop(listener);
    format!("http://127.0.0.1:{port}")
}

/// The head and the `Content-Length` bytes of body of one request, or
/// `None` when the client sent no request line.
fn read_request(stream: &TcpStream) -> Option<StubRequest> {
    let mut reader = BufReader::new(stream);
    let mut head = String::new();
    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            content_length = value.trim().parse().unwrap_or(0);
        }
        head.push_str(&line);
    }
    let mut request_line = head.lines().next()?.split(' ');
    let method = request_line.next()?.to_string();
    let target = request_line.next().unwrap_or_default().to_string();
    let mut body = vec![0; content_length];
    let _ = reader.read_exact(&mut body);
    Some(StubRequest {
        method,
        target,
        head,
        body,
    })
}

fn write_response(mut stream: TcpStream, response: &StubResponse) {
    let mut head = format!("HTTP/1.1 {} {}\r\n", response.status, response.status_text);
    for (name, value) in &response.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str(&format!(
        "Content-Length: {}\r\nConnection: close\r\n\r\n",
        response.body.len()
    ));
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&response.body);
    let _ = stream.flush();
}
