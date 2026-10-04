//! Port of GitHub Desktop's `app/test/unit/git/environment-test.ts`.
//!
//! GitHub Desktop's `envForProxy(remoteUrl, env, resolve)`
//! (`lib/git/environment.ts`) adds `http_proxy` / `https_proxy` for a
//! remote operation from the system proxy (`resolveGitProxy`, Electron's
//! `session.resolveProxy`) unless the environment already names one. It is
//! `corvene_git::env_for_proxy`. A rejected resolver promise is `Err`, an
//! `undefined` result `Ok(None)`.

use std::collections::HashMap;

use corvene_git::env_for_proxy;

const HTTP_PROXY_URL: &str = "http://proxy:8888/";
const HTTPS_PROXY_URL: &str = "https://proxy:8888/";

fn null_resolver(_url: &str) -> Result<Option<String>, String> {
    Ok(None)
}

fn throwing_resolver(_url: &str) -> Result<Option<String>, String> {
    Err("such error".to_string())
}

fn default_resolver(url: &str) -> Result<Option<String>, String> {
    if url.starts_with("http://") {
        Ok(Some(HTTP_PROXY_URL.to_string()))
    } else if url.starts_with("https://") {
        Ok(Some(HTTPS_PROXY_URL.to_string()))
    } else {
        Ok(None)
    }
}

fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

// GHD: unit/git/environment-test.ts › git/environmnent › envForProxy › sets the correct environment variable based on protocol
#[test]
fn sets_the_correct_environment_variable_based_on_protocol() {
    assert_eq!(
        env_for_proxy("https://github.com/", &env(&[]), &default_resolver),
        Some(env(&[("https_proxy", HTTPS_PROXY_URL)]))
    );

    assert_eq!(
        env_for_proxy("http://github.com/", &env(&[]), &default_resolver),
        Some(env(&[("http_proxy", HTTP_PROXY_URL)]))
    );
}

// GHD: unit/git/environment-test.ts › git/environmnent › envForProxy › fails gracefully if resolver throws
#[test]
fn fails_gracefully_if_resolver_throws() {
    assert_eq!(
        env_for_proxy("https://github.com/", &env(&[]), &throwing_resolver),
        None
    );
}

// GHD: unit/git/environment-test.ts › git/environmnent › envForProxy › it doesn't set any variables if resolver returns undefined
#[test]
fn it_doesnt_set_any_variables_if_resolver_returns_undefined() {
    assert_eq!(
        env_for_proxy("https://github.com/", &env(&[]), &null_resolver),
        None
    );
}

// GHD: unit/git/environment-test.ts › git/environmnent › envForProxy › sets the correct environment variable based on protocol #2
#[test]
fn sets_the_correct_environment_variable_based_on_protocol_2() {
    assert_eq!(
        env_for_proxy("https://github.com/", &env(&[]), &default_resolver),
        Some(env(&[("https_proxy", HTTPS_PROXY_URL)]))
    );

    assert_eq!(
        env_for_proxy("http://github.com/", &env(&[]), &default_resolver),
        Some(env(&[("http_proxy", HTTP_PROXY_URL)]))
    );
}

// GHD: unit/git/environment-test.ts › git/environmnent › envForProxy › ignores unknown protocols
#[test]
fn ignores_unknown_protocols() {
    assert_eq!(
        env_for_proxy("ftp://github.com/", &env(&[]), &default_resolver),
        None
    );
}

// GHD: unit/git/environment-test.ts › git/environmnent › envForProxy › does not override existing environment variables
#[test]
fn does_not_override_existing_environment_variables() {
    assert_eq!(
        env_for_proxy(
            "https://github.com/",
            &env(&[("https_proxy", "foo")]),
            &default_resolver
        ),
        None
    );

    assert_eq!(
        env_for_proxy(
            "https://github.com/",
            &env(&[("HTTPS_PROXY", "foo")]),
            &default_resolver
        ),
        None
    );

    assert_eq!(
        env_for_proxy(
            "http://github.com/",
            &env(&[("http_proxy", "foo")]),
            &default_resolver
        ),
        None
    );

    assert_eq!(
        env_for_proxy(
            "https://github.com/",
            &env(&[("ALL_PROXY", "foo")]),
            &default_resolver
        ),
        None
    );

    assert_eq!(
        env_for_proxy(
            "https://github.com/",
            &env(&[("all_proxy", "foo")]),
            &default_resolver
        ),
        None
    );
}
