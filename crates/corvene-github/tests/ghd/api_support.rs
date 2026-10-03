//! Helpers shared by the `api` lane's modules of `corvene-github`.
//!
//! GitHub Desktop's API tests hand `API` / `APIError` a `Response` built in
//! the test (`new Response(body, { status, statusText, headers })`, or a
//! stubbed `request` that resolves to one). Corvene's `Client` reads its
//! responses from the network, so [`serve`] answers its requests from a
//! local HTTP server with that same response, and [`unreachable_endpoint`]
//! stands for a `request` that throws (nothing listens there).

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::thread;

/// GitHub Desktop's `new Response(body, { status, statusText, headers })`.
pub struct StubResponse {
    pub status: u16,
    pub status_text: &'static str,
    pub headers: Vec<(&'static str, &'static str)>,
    pub body: String,
}

impl StubResponse {
    /// `new Response(body, { status })` with the status's usual reason text.
    pub fn new(status: u16, body: impl Into<String>) -> Self {
        let status_text = match status {
            200 => "OK",
            401 => "Unauthorized",
            402 => "Payment Required",
            403 => "Forbidden",
            404 => "Not Found",
            422 => "Unprocessable Entity",
            500 => "Internal Server Error",
            _ => "",
        };
        Self {
            status,
            status_text,
            headers: Vec::new(),
            body: body.into(),
        }
    }
}

/// Answer every request with `response` until the test binary exits;
/// returns the server's base URL (`http://127.0.0.1:<port>`), to be used as
/// the API endpoint.
pub fn serve(response: StubResponse) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut content_length = 0usize;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
                if let Some((name, value)) = line.split_once(':')
                    && name.eq_ignore_ascii_case("content-length")
                {
                    content_length = value.trim().parse().unwrap_or(0);
                }
            }
            let mut body = vec![0; content_length];
            let _ = std::io::Read::read_exact(&mut reader, &mut body);
            let mut head = format!("HTTP/1.1 {} {}\r\n", response.status, response.status_text);
            for (name, value) in &response.headers {
                head.push_str(&format!("{name}: {value}\r\n"));
            }
            head.push_str(&format!(
                "Content-Length: {}\r\nConnection: close\r\n\r\n",
                response.body.len()
            ));
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(response.body.as_bytes());
        }
    });
    format!("http://127.0.0.1:{port}")
}

/// A base URL nothing listens on: every request to it fails before a
/// response arrives (GitHub Desktop's stubbed `request` that throws).
pub fn unreachable_endpoint() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    format!("http://127.0.0.1:{port}")
}
