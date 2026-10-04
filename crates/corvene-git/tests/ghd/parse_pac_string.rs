//! Port of GitHub Desktop's `app/test/unit/parse-pac-string-test.ts`.
//!
//! GitHub Desktop's `parsePACString(pacString)` (`lib/parse-pac-string.ts`)
//! turns the PAC string of the system proxy for a URL ("PROXY host:port;
//! DIRECT") into cURL proxy URLs for git (`resolveGitProxy`, then
//! `envForProxy`). It is `corvene_git::parse_pac_string` (`None` for
//! GitHub Desktop's `null`).

use corvene_git::parse_pac_string;

fn urls(list: &[&str]) -> Option<Vec<String>> {
    Some(list.iter().map(|u| u.to_string()).collect())
}

// GHD: unit/parse-pac-string-test.ts › parsePACString › returns no url for DIRECT
#[test]
fn returns_no_url_for_direct() {
    assert!(parse_pac_string("DIRECT").is_none());
}

// GHD: unit/parse-pac-string-test.ts › parsePACString › parses Chromium PAC strings
#[test]
fn parses_chromium_pac_strings() {
    assert_eq!(
        parse_pac_string("PROXY myproxy:80;DIRECT"),
        urls(&["http://myproxy:80"])
    );

    assert_eq!(
        parse_pac_string("PROXY myproxy:80"),
        urls(&["http://myproxy:80"])
    );
    assert_eq!(
        parse_pac_string("PROXY myproxy:80; HTTPS secureproxy:443"),
        urls(&["http://myproxy:80", "https://secureproxy:443"])
    );

    assert_eq!(
        parse_pac_string("PROXY a:1;HTTP b:2;HTTPS c:3;SOCKS d:4;SOCKS4 e:5;SOCKS5 f:5;DIRECT"),
        urls(&[
            "http://a:1",
            "http://b:2",
            "https://c:3",
            "socks4://d:4",
            "socks4://e:5",
            "socks5://f:5",
        ])
    );
}

// not strictly necessary as Chromium doesn't add space inbetween
// GHD: unit/parse-pac-string-test.ts › parsePACString › parses PAC strings with white space between specs
#[test]
fn parses_pac_strings_with_white_space_between_specs() {
    assert_eq!(
        parse_pac_string(
            "PROXY a:1; HTTP b:2 ;\tHTTPS c:3\t;\tSOCKS d:4 ; SOCKS4 e:5  ;  SOCKS5 f:5  ; DIRECT"
        ),
        urls(&[
            "http://a:1",
            "http://b:2",
            "https://c:3",
            "socks4://d:4",
            "socks4://e:5",
            "socks5://f:5",
        ])
    );
}

// GHD: unit/parse-pac-string-test.ts › parsePACString › skips protocols cURL doesn't understand
#[test]
fn skips_protocols_curl_doesnt_understand() {
    let urls_found = parse_pac_string("QUIC qhost:1;PROXY phost:2;DIRECT");
    assert_eq!(urls_found, urls(&["http://phost:2"]));
}

// GHD: unit/parse-pac-string-test.ts › parsePACString › skips invalid specs
#[test]
fn skips_invalid_specs() {
    let urls_found = parse_pac_string("PROXY;HTTPS;DIRECT");
    assert!(urls_found.is_none());
}
