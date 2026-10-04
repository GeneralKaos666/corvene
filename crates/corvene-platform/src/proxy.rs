//! The proxy for Corvene's own HTTPS requests (the GitHub API, sign-in,
//! avatars, updates, packs, extensions). GHD makes them through Chromium,
//! which follows the system's proxy settings; ureq on its own reads only the
//! proxy environment variables, which an app started from the Dock or Finder
//! does not have (desktop#13975).
//!
//! [`agent_proxy`] takes, in order: the environment (`ALL_PROXY`,
//! `HTTPS_PROXY`, `HTTP_PROXY`, with `NO_PROXY`: what ureq reads by itself),
//! git's global `http.proxy` (handed over with [`set_git_http_proxy`]: only
//! corvene-git runs git), and on macOS the HTTPS (else HTTP) proxy of the
//! network settings with its bypass list (`SCDynamicStoreCopyProxies`).
//! Deviation (`.docs/deviations.md`): proxy auto-configuration (PAC) files
//! and SOCKS proxies are not used, and Windows and Linux stop after
//! `http.proxy`. git's own remote operations are a separate matter
//! (`corvene_git::proxy`).

use std::sync::RwLock;

use tracing::warn;

static GIT_HTTP_PROXY: RwLock<Option<String>> = RwLock::new(None);

/// git's global `http.proxy`, read by the core at launch.
pub fn set_git_http_proxy(proxy: Option<String>) {
    if let Ok(mut slot) = GIT_HTTP_PROXY.write() {
        *slot = proxy.filter(|p| !p.trim().is_empty());
    }
}

fn git_http_proxy() -> Option<String> {
    GIT_HTTP_PROXY.read().ok().and_then(|slot| slot.clone())
}

/// The proxy a ureq agent should use (`config_builder().proxy(…)`); `None`
/// connects directly.
pub fn agent_proxy() -> Option<ureq::Proxy> {
    if let Some(proxy) = ureq::Proxy::try_from_env() {
        return Some(proxy);
    }
    if let Some(url) = git_http_proxy() {
        match ureq::Proxy::new(&proxy_url(&url)) {
            Ok(proxy) => return Some(proxy),
            Err(err) => warn!(%err, "ignoring git's http.proxy"),
        }
    }
    let system = imp::system_proxy()?;
    let mut builder = ureq::Proxy::builder(ureq::ProxyProtocol::Http)
        .host(&system.host)
        .port(system.port);
    // Chromium never sends loopback requests to a proxy; macOS bypass
    // entries (`*.local`, `.example.com`, `example.com`) are `NO_PROXY`
    // entries too (ureq drops the ones with a `/` mask)
    for exception in ["localhost", "127.0.0.1", "::1"]
        .into_iter()
        .chain(system.exceptions.iter().map(String::as_str))
    {
        builder = builder.no_proxy(exception.trim());
    }
    match builder.build() {
        Ok(proxy) => Some(proxy),
        Err(err) => {
            warn!(%err, host = %system.host, "ignoring the system proxy");
            None
        }
    }
}

/// git's `http.proxy` takes `host:port` without a scheme (cURL's default,
/// `http://`); ureq wants one.
fn proxy_url(value: &str) -> String {
    let value = value.trim();
    if value.contains("://") {
        value.to_string()
    } else {
        format!("http://{value}")
    }
}

/// The system's HTTP(S) proxy and the hosts that bypass it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemProxy {
    pub host: String,
    pub port: u16,
    pub exceptions: Vec<String>,
}

#[cfg(target_os = "macos")]
mod imp {
    #![allow(unexpected_cfgs)] // `objc` macros probe a `cargo-clippy` feature

    use std::ffi::{CStr, CString, c_char, c_void};

    use objc::runtime::{BOOL, NO, Object};
    use objc::{class, msg_send, sel, sel_impl};

    use super::SystemProxy;

    #[link(name = "SystemConfiguration", kind = "framework")]
    unsafe extern "C" {
        /// Returns a retained `CFDictionaryRef` (toll-free an `NSDictionary`).
        fn SCDynamicStoreCopyProxies(store: *const c_void) -> *mut Object;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(cf: *const c_void);
    }

    pub fn system_proxy() -> Option<SystemProxy> {
        // SAFETY: `SCDynamicStoreCopyProxies(NULL)` returns a dictionary we
        // own (released below) or NULL; the Foundation message sends read it
        // and every string is copied out before the pool is drained.
        unsafe {
            let pool: *mut Object = msg_send![class!(NSAutoreleasePool), new];
            let settings = SCDynamicStoreCopyProxies(std::ptr::null());
            let proxy = if settings.is_null() {
                None
            } else {
                let found = ["HTTPS", "HTTP"].iter().find_map(|kind| {
                    let enabled = number(settings, &format!("{kind}Enable"))? != 0;
                    let host = string(settings, &format!("{kind}Proxy"))?;
                    let port = u16::try_from(number(settings, &format!("{kind}Port"))?).ok()?;
                    (enabled && !host.is_empty() && port != 0).then_some((host, port))
                });
                let proxy = found.map(|(host, port)| SystemProxy {
                    host,
                    port,
                    exceptions: strings(settings, "ExceptionsList"),
                });
                CFRelease(settings.cast());
                proxy
            };
            let _: () = msg_send![pool, drain];
            proxy
        }
    }

    /// `[dict objectForKey:key]`.
    unsafe fn value(dict: *mut Object, key: &str) -> Option<*mut Object> {
        let key = CString::new(key).ok()?;
        // SAFETY: the caller passes a live dictionary; the key string is
        // autoreleased
        unsafe {
            let ns_key: *mut Object =
                msg_send![class!(NSString), stringWithUTF8String: key.as_ptr()];
            let value: *mut Object = msg_send![dict, objectForKey: ns_key];
            (!value.is_null()).then_some(value)
        }
    }

    unsafe fn number(dict: *mut Object, key: &str) -> Option<i64> {
        // SAFETY: a value of the proxy settings; `NSNumber` answers
        // `longLongValue`, and so does an `NSString` of digits
        unsafe {
            let value = value(dict, key)?;
            let responds: BOOL = msg_send![value, respondsToSelector: sel!(longLongValue)];
            (responds != NO).then(|| msg_send![value, longLongValue])
        }
    }

    unsafe fn ns_string(value: *mut Object) -> Option<String> {
        // SAFETY: `value` is a live object; only strings are converted
        unsafe {
            let is_string: BOOL = msg_send![value, isKindOfClass: class!(NSString)];
            if is_string == NO {
                return None;
            }
            let utf8: *const c_char = msg_send![value, UTF8String];
            (!utf8.is_null()).then(|| CStr::from_ptr(utf8).to_string_lossy().into_owned())
        }
    }

    unsafe fn string(dict: *mut Object, key: &str) -> Option<String> {
        // SAFETY: as in `value`
        unsafe { ns_string(value(dict, key)?) }
    }

    unsafe fn strings(dict: *mut Object, key: &str) -> Vec<String> {
        // SAFETY: as in `value`; an `NSArray` is walked by index
        unsafe {
            let Some(array) = value(dict, key) else {
                return Vec::new();
            };
            let is_array: BOOL = msg_send![array, isKindOfClass: class!(NSArray)];
            if is_array == NO {
                return Vec::new();
            }
            let count: usize = msg_send![array, count];
            (0..count)
                .filter_map(|i| {
                    let item: *mut Object = msg_send![array, objectAtIndex: i];
                    ns_string(item)
                })
                .collect()
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::SystemProxy;

    pub fn system_proxy() -> Option<SystemProxy> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_proxy_values_get_a_scheme() {
        assert_eq!(proxy_url("proxy.corp:3128"), "http://proxy.corp:3128");
        assert_eq!(proxy_url(" https://p:1 "), "https://p:1");
        assert_eq!(proxy_url("socks5://p:1080"), "socks5://p:1080");
    }

    #[test]
    fn system_proxy_lookup_does_not_crash() {
        // whatever the machine's settings are
        let _ = imp::system_proxy();
    }
}
