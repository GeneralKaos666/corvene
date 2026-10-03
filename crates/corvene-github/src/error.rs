//! Errors of the GitHub API client (`lib/http.ts` `APIError`).

#[derive(Debug, thiserror::Error)]
pub enum GitHubError {
    #[error("network error: {0}")]
    Http(#[from] ureq::Error),
    #[error("unexpected response: {0}")]
    Json(#[from] serde_json::Error),
    /// GHD `APIError`: the server answered with an error status.
    #[error("GitHub returned {status}: {message}")]
    Api {
        /// `APIError.responseStatus`
        status: u16,
        /// `APIError.message`
        message: String,
        /// `APIError.apiError`: the error body, `None` when it could not be
        /// parsed.
        api_error: Option<ApiErrorBody>,
        /// A 401 that means the token was revoked (GHD `ghRequest`'s
        /// `emitTokenInvalidated`): it came from GitHub (`X-GitHub-Request-Id`)
        /// and is not a two-factor challenge (`X-GitHub-OTP`).
        token_invalidated: bool,
    },
    #[error("authentication failed: {0}")]
    Auth(String),
    #[error("the sign-in request expired; try again")]
    Expired,
    #[error("sign-in was denied")]
    Denied,
}

impl GitHubError {
    /// An `Api` error without a parsed body (statuses Corvene reports itself).
    pub fn api(status: u16, message: impl Into<String>) -> Self {
        GitHubError::Api {
            status,
            message: message.into(),
            api_error: None,
            token_invalidated: false,
        }
    }

    /// `APIError.responseStatus`, for an error the server answered.
    pub fn status(&self) -> Option<u16> {
        match self {
            GitHubError::Api { status, .. } => Some(*status),
            _ => None,
        }
    }

    /// `APIError.apiError`: the parsed error body.
    pub fn api_error(&self) -> Option<&ApiErrorBody> {
        match self {
            GitHubError::Api { api_error, .. } => api_error.as_ref(),
            _ => None,
        }
    }

    /// The account's token was revoked (GHD `emitTokenInvalidated`): show
    /// `InvalidatedToken`.
    pub fn is_token_invalidated(&self) -> bool {
        matches!(
            self,
            GitHubError::Api {
                token_invalidated: true,
                ..
            }
        )
    }
}

/// GHD `IAPIError`: the body of an error response.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ApiErrorBody {
    pub message: Option<String>,
    pub errors: Vec<ApiErrorItem>,
}

/// GHD `IError`: one of an error body's `errors` (validation failures).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ApiErrorItem {
    pub message: Option<String>,
    pub resource: Option<String>,
    pub field: Option<String>,
    pub code: Option<String>,
}

impl ApiErrorBody {
    /// The parsed body of an error response (GHD `deserialize<IAPIError>`):
    /// `None` when it is not JSON or is JSON `null` (GHD's `apiError` is
    /// then `null`). GHD reads the fields off whatever JSON came back, so a
    /// field of another type is left out rather than failing the parse.
    pub fn parse(body: &[u8]) -> Option<Self> {
        let value: serde_json::Value = serde_json::from_slice(body).ok()?;
        if value.is_null() {
            return None;
        }
        let text = |v: &serde_json::Value, key: &str| {
            v.get(key).and_then(|s| s.as_str()).map(str::to_string)
        };
        let errors = value
            .get("errors")
            .and_then(|e| e.as_array())
            .map(|items| {
                items
                    .iter()
                    .map(|item| ApiErrorItem {
                        message: text(item, "message"),
                        resource: text(item, "resource"),
                        field: text(item, "field"),
                        code: text(item, "code"),
                    })
                    .collect()
            })
            .unwrap_or_default();
        Some(Self {
            message: text(&value, "message"),
            errors,
        })
    }

    /// `APIError.message` from this body: the top-level `message` followed by
    /// the `errors[].message`s in parentheses ("Validation Failed (name
    /// already exists)"); `None` without a top-level message. With
    /// `details` (flag `api-error-details`, desktop/desktop#19465), an
    /// error without a message shows as `field code` instead of nothing,
    /// and the errors stand alone when there is no top-level message.
    pub fn message(&self, details: bool) -> Option<String> {
        let message = self.message.as_deref().filter(|m| !m.is_empty());
        if message.is_none() && !details {
            return None;
        }
        let items: Vec<String> = self
            .errors
            .iter()
            .filter_map(|e| match (&e.message, &e.field, &e.code) {
                (Some(message), _, _) if !message.is_empty() => Some(message.clone()),
                (_, Some(field), Some(code)) if details => Some(format!("{field} {code}")),
                // GHD joins `undefined` as an empty string
                _ if details => None,
                _ => Some(String::new()),
            })
            .collect();
        let additional = items.join(", ");
        match (message, additional.is_empty()) {
            (None, true) => None,
            (None, false) => Some(additional),
            (Some(message), true) => Some(message.to_string()),
            (Some(message), false) => Some(format!("{message} ({additional})")),
        }
    }
}

pub type Result<T> = std::result::Result<T, GitHubError>;
