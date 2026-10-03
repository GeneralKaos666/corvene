//! Port of GitHub Desktop's `app/test/unit/enterprise-validate-url-test.ts`.
//!
//! GitHub Desktop's `validateURL(address)` (`ui/lib/enterprise-validate-url.ts`)
//! checks the GitHub Enterprise address typed in the sign-in dialog and
//! returns it (with `https://` prepended when it has no scheme) or throws.
//! Corvene's counterpart is `corvene_github::Endpoint::enterprise(address,
//! allow_http)`, `Endpoint::validate_enterprise(..).ok()` (the sign-in
//! store's `set_endpoint` calls `validate_enterprise` on the typed address
//! and shows its error): it returns the endpoint, whose `web_base` is the
//! validated URL, or `None` where `validateURL` throws.
//! `allow_http` is the value of `314-enterprise-plain-http` in the
//! `github-desktop` preset (off; the flag is off in every preset).

use corvene_github::Endpoint;

/// `314-enterprise-plain-http` in the `github-desktop` preset.
const ENTERPRISE_PLAIN_HTTP: bool = false;

/// `validateURL(address)`: `Some(url)` for the returned URL, `None` for a
/// throw.
fn validate_url(address: &str) -> Option<String> {
    Endpoint::enterprise(address, ENTERPRISE_PLAIN_HTTP).map(|endpoint| endpoint.web_base)
}

// GHD: unit/enterprise-validate-url-test.ts › validateURL › passes through a valid url
#[test]
fn passes_through_a_valid_url() {
    let url = "https://ghe.io:9000";
    let result = validate_url(url);
    assert_eq!(result.as_deref(), Some(url));
}

// GHD: unit/enterprise-validate-url-test.ts › validateURL › prepends https if no protocol is provided
#[test]
fn prepends_https_if_no_protocol_is_provided() {
    let url = validate_url("ghe.io");
    assert_eq!(url.as_deref(), Some("https://ghe.io"));
}

// GHD: unit/enterprise-validate-url-test.ts › validateURL › throws if given an invalid protocol
#[test]
fn throws_if_given_an_invalid_protocol() {
    assert_eq!(validate_url("ftp://ghe.io"), None);
}

// GHD: unit/enterprise-validate-url-test.ts › validateURL › throws if given whitespace
#[test]
fn throws_if_given_whitespace() {
    assert_eq!(validate_url("    "), None);
}

// GHD: unit/enterprise-validate-url-test.ts › validateURL › handles whitespace alongside valid text
#[test]
fn handles_whitespace_alongside_valid_text() {
    let url = validate_url("ghe.io   ");
    assert_eq!(url.as_deref(), Some("https://ghe.io"));
}
