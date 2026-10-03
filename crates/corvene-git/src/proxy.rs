//! The proxy git's remote operations use: GHD `envForProxy`
//! (`lib/git/environment.ts`), `resolveGitProxy` (`lib/resolve-git-proxy.ts`)
//! and `parsePACString` (`lib/parse-pac-string.ts`).
//!
//! GHD asks Electron (Chromium's system proxy resolution: the OS proxy
//! settings, PAC scripts, WPAD) for the PAC string of a remote's URL, turns
//! the first proxy it names into `http_proxy` / `https_proxy` for git, and
//! leaves the environment alone when the user already set one. Corvene has
//! no system proxy lookup yet (`.docs/TODO.md` › Platform):
//! [`set_system_proxy_resolver`] is where the platform plugs one in. Until
//! it does, [`env_for_remote_operation`] adds nothing and git uses the proxy
//! its environment and configuration name, as before.

use std::collections::HashMap;
use std::sync::RwLock;

use tracing::{info, warn};

/// The PAC string (`"PROXY host:port; DIRECT"`) of the system proxy for a
/// URL: Electron's `session.resolveProxy`, which GHD's `resolveGitProxy`
/// calls.
pub type SystemProxyResolver = fn(&str) -> Result<String, String>;

static SYSTEM_PROXY_RESOLVER: RwLock<Option<SystemProxyResolver>> = RwLock::new(None);

/// Sets (or clears) the system proxy lookup [`env_for_remote_operation`]
/// uses.
pub fn set_system_proxy_resolver(resolver: Option<SystemProxyResolver>) {
    if let Ok(mut slot) = SYSTEM_PROXY_RESOLVER.write() {
        *slot = resolver;
    }
}

fn system_proxy_resolver() -> Option<SystemProxyResolver> {
    SYSTEM_PROXY_RESOLVER.read().ok().and_then(|slot| *slot)
}

/// GHD `parsePACString`: the cURL proxy URLs of a PAC string as Chromium
/// writes it (`ProxyList::ToPacString`), in order, up to the first `DIRECT`;
/// `None` when none is left. Protocols cURL does not speak (`QUIC`) and specs
/// without a host are skipped; the port is kept as given (cURL's default
/// proxy port is 1080, not the protocol's).
pub fn parse_pac_string(pac_string: &str) -> Option<Vec<String>> {
    // the happy path, for nearly everyone
    if pac_string == "DIRECT" {
        return None;
    }
    let mut urls = Vec::new();
    for spec in pac_string.trim().split(';').map(str::trim) {
        // cURL has no fallback to a direct connection: stop at the first
        if spec
            .get(..6)
            .is_some_and(|start| start.eq_ignore_ascii_case("direct"))
        {
            break;
        }
        // `spec.split(/\s+/, 2)`: the type and the first word after it
        let mut words = spec.split_whitespace();
        let (Some(protocol), Some(endpoint)) = (words.next(), words.next()) else {
            continue;
        };
        match url_from_protocol_and_endpoint(protocol, endpoint) {
            Some(url) => urls.push(url),
            None => warn!(spec, "skipping proxy spec"),
        }
    }
    (!urls.is_empty()).then_some(urls)
}

/// GHD `urlFromProtocolAndEndpoint`. `HTTP` is an alias for `PROXY`,
/// `SOCKS` for `SOCKS4`.
fn url_from_protocol_and_endpoint(protocol: &str, endpoint: &str) -> Option<String> {
    let scheme = match protocol.to_lowercase().as_str() {
        "proxy" | "http" => "http",
        "https" => "https",
        "socks" | "socks4" => "socks4",
        "socks5" => "socks5",
        _ => return None,
    };
    Some(format!("{scheme}://{endpoint}"))
}

/// GHD `resolveGitProxy`: the first proxy of the PAC string `resolve_proxy`
/// gives for `url` (a failing lookup counts as `DIRECT`). Windows skips
/// `https://` proxies, which git's `schannel` backend cannot use.
pub fn resolve_git_proxy(
    url: &str,
    resolve_proxy: impl Fn(&str) -> Result<String, String>,
) -> Option<String> {
    let pac_string = resolve_proxy(url).unwrap_or_else(|err| {
        warn!(url, %err, "failed resolving proxy");
        "DIRECT".to_string()
    });
    parse_pac_string(&pac_string)?.into_iter().find(|proxy| {
        let unsupported = cfg!(windows) && proxy.starts_with("https://");
        if unsupported {
            warn!("ignoring https proxy, not supported in cURL/schannel");
        }
        !unsupported
    })
}

/// GHD `envForProxy(remoteUrl, env, resolve)`: `http_proxy` or
/// `https_proxy` (by the remote URL's scheme, case-insensitive) set to what
/// `resolve` finds for the URL. `None` for any other scheme (only cURL
/// speaks to proxies; ssh and `git://` do not), when `ALL_PROXY` /
/// `all_proxy` or the scheme's own variable (for https also `HTTPS_PROXY`)
/// is already in `env`, and when `resolve` fails or finds nothing.
pub fn env_for_proxy(
    remote_url: &str,
    env: &HashMap<String, String>,
    resolve: &dyn Fn(&str) -> Result<Option<String>, String>,
) -> Option<HashMap<String, String>> {
    // `/^(https?):\/\//i`
    let scheme_end = remote_url.find("://")?;
    let protocol = remote_url[..scheme_end].to_ascii_lowercase();
    if protocol != "http" && protocol != "https" {
        return None;
    }
    // whoever set ALL_PROXY knows what they want: cURL reads both cases
    if env.contains_key("ALL_PROXY") || env.contains_key("all_proxy") {
        info!("proxy url not resolved, ALL_PROXY already set");
        return None;
    }
    // cURL reads `http_proxy` in lower case only
    let env_key = format!("{protocol}_proxy");
    if env.contains_key(&env_key) || (protocol == "https" && env.contains_key("HTTPS_PROXY")) {
        info!("proxy url not resolved, {env_key} already set");
        return None;
    }
    let proxy_url = resolve(remote_url).unwrap_or_else(|err| {
        warn!(%err, "failed resolving git proxy");
        None
    })?;
    Some(HashMap::from([(env_key, proxy_url)]))
}

/// GHD `envForRemoteOperation(remoteUrl)`'s proxy half: [`env_for_proxy`]
/// with the process environment and the system proxy, for the git command
/// of a remote operation on `remote_url`. Empty while no system proxy lookup
/// is set ([`set_system_proxy_resolver`]).
pub fn env_for_remote_operation(remote_url: &str) -> Vec<(String, String)> {
    let Some(system) = system_proxy_resolver() else {
        return Vec::new();
    };
    let mut env = HashMap::new();
    for (key, value) in std::env::vars_os() {
        let key = key.to_string_lossy().into_owned();
        let value = value.to_string_lossy().into_owned();
        // Windows: Node's `process.env` (and cURL) ignore the case of names
        if cfg!(windows) {
            env.insert(key.to_lowercase(), value.clone());
            env.insert(key.to_uppercase(), value.clone());
        }
        env.insert(key, value);
    }
    env_for_proxy(remote_url, &env, &|url| Ok(resolve_git_proxy(url, system)))
        .map(|vars| vars.into_iter().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_the_first_usable_proxy() {
        let pac = |_: &str| Ok("SOCKS5 a:1; PROXY b:2".to_string());
        assert_eq!(
            resolve_git_proxy("https://x", pac).as_deref(),
            Some("socks5://a:1")
        );
        let failing = |_: &str| Err("no".to_string());
        assert_eq!(resolve_git_proxy("https://x", failing), None);
    }

    #[test]
    fn no_resolver_no_variables() {
        set_system_proxy_resolver(None);
        assert!(env_for_remote_operation("https://github.com/a/b.git").is_empty());
    }
}
