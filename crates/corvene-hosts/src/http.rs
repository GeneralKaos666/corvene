//! The blocking HTTP side every provider shares: one `ureq` agent with the
//! system proxy, the host's way of sending a token, JSON in and out, error
//! bodies turned into messages, and `Link: <…>; rel="next"` paging (GitLab
//! and Gitea; Bitbucket pages through a `next` field, see `bitbucket.rs`).

use std::time::Duration;

use corvene_models::HostEndpoint;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tracing::debug;

use crate::{HostError, Result};

/// How a request carries the account's token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Auth {
    /// No token: public data only.
    Anonymous,
    /// `Authorization: Bearer <token>` (GitLab, OAuth tokens everywhere).
    Bearer(String),
    /// `Authorization: token <token>` (Gitea / Forgejo access tokens).
    Token(String),
    /// `Authorization: Basic base64(user:token)` (Bitbucket API tokens).
    Basic { user: String, token: String },
}

impl Auth {
    fn header(&self) -> Option<String> {
        match self {
            Auth::Anonymous => None,
            Auth::Bearer(token) => Some(format!("Bearer {token}")),
            Auth::Token(token) => Some(format!("token {token}")),
            Auth::Basic { user, token } => Some(format!(
                "Basic {}",
                base64(format!("{user}:{token}").as_bytes())
            )),
        }
    }
}

/// Standard base64 with padding (`Authorization: Basic`).
pub(crate) fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[((n >> (18 - 6 * i)) & 0x3f) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Percent-encode one URL component (RFC 3986 unreserved characters stay),
/// e.g. a GitLab project path `group/sub/proj` → `group%2Fsub%2Fproj`.
pub fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// A response's headers as `(lower-case name, value)` pairs.
pub type Headers = Vec<(String, String)>;

fn header_pairs(map: &ureq::http::HeaderMap) -> Headers {
    map.iter()
        .filter_map(|(name, value)| {
            Some((
                name.as_str().to_ascii_lowercase(),
                value.to_str().ok()?.to_string(),
            ))
        })
        .collect()
}

/// The URL of a `Link` header's `rel="next"` entry.
pub fn next_link(headers: &Headers) -> Option<String> {
    let link = headers
        .iter()
        .find(|(name, _)| name == "link")
        .map(|(_, v)| v)?;
    link.split(',').find_map(|part| {
        let (url, params) = part.split_once(';')?;
        params
            .split(';')
            .any(|p| {
                let p = p.trim().replace(' ', "");
                p == "rel=\"next\"" || p == "rel=next"
            })
            .then(|| {
                url.trim()
                    .trim_start_matches('<')
                    .trim_end_matches('>')
                    .to_string()
            })
    })
}

/// The message of an error body, whatever shape the host uses:
/// GitLab `{"message": "…"}` or `{"message": {"field": ["…"]}}` or
/// `{"error": "…", "error_description": "…"}`, Gitea `{"message": "…"}`,
/// Bitbucket `{"type": "error", "error": {"message": "…", "detail": "…"}}`.
pub fn error_message(body: &[u8]) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    let text = |v: &serde_json::Value| v.as_str().map(str::to_string);
    match value.get("message") {
        Some(serde_json::Value::String(s)) if !s.is_empty() => return Some(s.clone()),
        Some(serde_json::Value::Object(fields)) => {
            let parts: Vec<String> = fields
                .iter()
                .map(|(field, errors)| {
                    let errors = match errors {
                        serde_json::Value::Array(items) => items
                            .iter()
                            .filter_map(|e| e.as_str())
                            .collect::<Vec<_>>()
                            .join(", "),
                        other => other.as_str().unwrap_or_default().to_string(),
                    };
                    format!("{field} {errors}").trim().to_string()
                })
                .collect();
            if !parts.is_empty() {
                return Some(parts.join("; "));
            }
        }
        Some(serde_json::Value::Array(items)) => {
            let parts: Vec<&str> = items.iter().filter_map(|e| e.as_str()).collect();
            if !parts.is_empty() {
                return Some(parts.join("; "));
            }
        }
        _ => {}
    }
    match value.get("error") {
        Some(serde_json::Value::Object(error)) => {
            let message = error.get("message").and_then(text)?;
            Some(match error.get("detail").and_then(text) {
                Some(detail) if !detail.is_empty() => format!("{message}: {detail}"),
                _ => message,
            })
        }
        Some(serde_json::Value::String(error)) => {
            Some(match value.get("error_description").and_then(text) {
                Some(description) if !description.is_empty() => description,
                _ => error.clone(),
            })
        }
        _ => None,
    }
}

/// One host's REST API with one way of authenticating.
pub struct Http {
    agent: ureq::Agent,
    pub endpoint: HostEndpoint,
    pub auth: Auth,
}

impl Http {
    pub fn new(endpoint: HostEndpoint, auth: Auth) -> Self {
        let agent = ureq::Agent::config_builder()
            // the system proxy too (`corvene_platform::proxy`)
            .proxy(corvene_platform::proxy::agent_proxy())
            .timeout_global(Some(Duration::from_secs(30)))
            .http_status_as_error(false)
            .user_agent(corvene_github::USER_AGENT)
            .build()
            .new_agent();
        Self {
            agent,
            endpoint,
            auth,
        }
    }

    fn error(&self, url: &str, mut response: ureq::http::Response<ureq::Body>) -> HostError {
        let status = response.status();
        let body = response.body_mut().read_to_vec().unwrap_or_default();
        let message = error_message(&body).unwrap_or_else(|| {
            format!(
                "{} returned {} ({url})",
                self.endpoint.friendly_name(),
                status.canonical_reason().unwrap_or("an error"),
            )
        });
        HostError::Api {
            status: status.as_u16(),
            message,
        }
    }

    fn authorized<B>(&self, request: ureq::RequestBuilder<B>) -> ureq::RequestBuilder<B> {
        match self.auth.header() {
            Some(header) => request.header("Authorization", &header),
            None => request,
        }
    }

    /// `GET path` → the decoded body and the response headers.
    pub fn get<T: DeserializeOwned>(&self, path: &str) -> Result<(T, Headers)> {
        let url = self.endpoint.api(path);
        debug!(%url, "GET");
        let request = self
            .authorized(self.agent.get(&url))
            .header("Accept", "application/json");
        let mut response = request.call()?;
        if !response.status().is_success() {
            return Err(self.error(&url, response));
        }
        let headers = header_pairs(response.headers());
        let body = response.body_mut().read_to_vec()?;
        Ok((serde_json::from_slice(&body)?, headers))
    }

    /// `GET path` → the decoded body; `None` on a 404.
    pub fn get_optional<T: DeserializeOwned>(&self, path: &str) -> Result<Option<T>> {
        match self.get(path) {
            Ok((value, _)) => Ok(Some(value)),
            Err(HostError::Api { status: 404, .. }) => Ok(None),
            Err(err) => Err(err),
        }
    }

    /// Every page of a list, following `Link: rel="next"`, at most
    /// `max_pages` pages.
    pub fn get_all<T: DeserializeOwned>(&self, path: &str, max_pages: usize) -> Result<Vec<T>> {
        let mut items = Vec::new();
        let mut next = Some(path.to_string());
        let mut pages = 0;
        while let Some(path) = next.take() {
            let (page, headers): (Vec<T>, _) = self.get(&path)?;
            items.extend(page);
            pages += 1;
            next = next_link(&headers).filter(|_| pages < max_pages);
        }
        Ok(items)
    }

    /// Every page of a list whose first page says how many there are
    /// (GitLab `x-total-pages`, Gitea `x-total-count` over `per_page`): the
    /// rest are read side by side, at most `max_pages` in all. Without those
    /// headers it follows `Link: rel="next"` like [`Http::get_all`].
    pub fn get_all_parallel<T: DeserializeOwned + Send>(
        &self,
        path: &str,
        per_page: usize,
        max_pages: usize,
    ) -> Result<Vec<T>> {
        let (first, headers): (Vec<T>, _) = self.get(path)?;
        let header = |name: &str| {
            headers
                .iter()
                .find(|(n, _)| n == name)
                .and_then(|(_, v)| v.trim().parse::<usize>().ok())
        };
        let pages = header("x-total-pages").or_else(|| {
            header("x-total-count").map(|total| total.div_ceil(per_page.max(1)))
        });
        let Some(pages) = pages.map(|p| p.min(max_pages)) else {
            let mut items = first;
            let mut next = next_link(&headers);
            let mut read = 1;
            while let Some(url) = next.take().filter(|_| read < max_pages) {
                let (page, headers): (Vec<T>, _) = self.get(&url)?;
                items.extend(page);
                read += 1;
                next = next_link(&headers);
            }
            return Ok(items);
        };
        let separator = if path.contains('?') { '&' } else { '?' };
        let rest: Vec<Result<Vec<T>>> = std::thread::scope(|scope| {
            let handles: Vec<_> = (2..=pages)
                .map(|page| {
                    let url = format!("{path}{separator}page={page}");
                    scope.spawn(move || self.get::<Vec<T>>(&url).map(|(items, _)| items))
                })
                .collect();
            handles
                .into_iter()
                .map(|h| {
                    h.join()
                        .unwrap_or_else(|_| Err(HostError::Auth("a page read failed".into())))
                })
                .collect()
        });
        let mut items = first;
        for page in rest {
            items.extend(page?);
        }
        Ok(items)
    }

    /// `POST path` with a JSON body → the decoded answer.
    pub fn post<B: Serialize, T: DeserializeOwned>(&self, path: &str, body: &B) -> Result<T> {
        let url = self.endpoint.api(path);
        debug!(%url, "POST");
        let request = self
            .authorized(self.agent.post(&url))
            .header("Accept", "application/json");
        let mut response = request.send_json(body)?;
        if !response.status().is_success() {
            return Err(self.error(&url, response));
        }
        let body = response.body_mut().read_to_vec()?;
        Ok(serde_json::from_slice(&body)?)
    }

    /// `POST url` as a form (OAuth token endpoints, absolute URLs), with
    /// the client's own authentication (Bitbucket's consumer key).
    pub fn post_form<T: DeserializeOwned>(&self, url: &str, form: &[(&str, &str)]) -> Result<T> {
        debug!(%url, "POST form");
        let mut response = self
            .authorized(self.agent.post(url))
            .header("Accept", "application/json")
            .send_form(form.iter().copied())?;
        if !response.status().is_success() {
            return Err(self.error(url, response));
        }
        let body = response.body_mut().read_to_vec()?;
        Ok(serde_json::from_slice(&body)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc_4648() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(
            Auth::Basic {
                user: "a@b.c".into(),
                token: "t".into()
            }
            .header()
            .unwrap(),
            "Basic YUBiLmM6dA=="
        );
    }

    #[test]
    fn follows_link_headers() {
        let headers = vec![(
            "link".to_string(),
            "<https://gitlab.com/api/v4/projects?page=2>; rel=\"next\", <https://gitlab.com/api/v4/projects?page=9>; rel=\"last\"".to_string(),
        )];
        assert_eq!(
            next_link(&headers).as_deref(),
            Some("https://gitlab.com/api/v4/projects?page=2")
        );
        let last = vec![("link".to_string(), "<x?page=1>; rel=\"first\"".to_string())];
        assert_eq!(next_link(&last), None);
    }

    #[test]
    fn reads_error_bodies() {
        assert_eq!(
            error_message(br#"{"message":"401 Unauthorized"}"#).as_deref(),
            Some("401 Unauthorized")
        );
        assert_eq!(
            error_message(br#"{"message":{"source_branch":["is invalid"]}}"#).as_deref(),
            Some("source_branch is invalid")
        );
        assert_eq!(
            error_message(br#"{"message":["Another open merge request already exists"]}"#)
                .as_deref(),
            Some("Another open merge request already exists")
        );
        assert_eq!(
            error_message(br#"{"type":"error","error":{"message":"Bad request","detail":"no"}}"#)
                .as_deref(),
            Some("Bad request: no")
        );
        assert_eq!(
            error_message(br#"{"error":"invalid_grant","error_description":"expired"}"#).as_deref(),
            Some("expired")
        );
        assert_eq!(error_message(b"<html>"), None);
        assert_eq!(encode("group/sub proj"), "group%2Fsub%20proj");
    }
}
