//! HTTPS for the registries and downloads: one `ureq` agent, a host
//! allow-list, size caps and a streaming sha256 for downloads. Everything an
//! extension fetches is untrusted; nothing here follows a redirect off
//! HTTPS.

use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

use sha2::{Digest, Sha256};
use ureq::ResponseExt;

use crate::ExtensionError;

const USER_AGENT: &str = concat!("Corvene/", env!("CARGO_PKG_VERSION"));

/// Hosts the registries and GitHub serve from. A user-typed URL adds its
/// own host for that one download ([`download`]'s `extra_host`).
pub const ALLOWED_HOSTS: &[&str] = &[
    "open-vsx.org",
    "api.zed.dev",
    "zed.dev",
    "api.pulsar-edit.dev",
    "github.com",
    "api.github.com",
    "codeload.github.com",
    "objects.githubusercontent.com",
    "raw.githubusercontent.com",
    "release-assets.githubusercontent.com",
    // Zed's archives are served from its object store
    "zed-extensions.nyc3.digitaloceanspaces.com",
];

/// A registry's JSON answer is capped here.
pub const MAX_JSON_BYTES: u64 = 8 * 1024 * 1024;
/// An extension archive is capped here.
pub const MAX_ARCHIVE_BYTES: u64 = 64 * 1024 * 1024;
/// A grammar's source tarball is capped here (parser.c can run to tens of
/// megabytes).
pub const MAX_SOURCE_BYTES: u64 = 128 * 1024 * 1024;

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(30)))
        .timeout_global(Some(Duration::from_secs(600)))
        .http_status_as_error(false)
        .https_only(true)
        .max_redirects(5)
        .user_agent(USER_AGENT)
        .build()
        .new_agent()
}

/// The host of `url`, lowercase, when it is an `https://` URL.
pub fn host_of(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://")?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let host = rest[..end].rsplit('@').next().unwrap_or("");
    let host = host.split(':').next().unwrap_or("");
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

fn allowed(url: &str, extra_host: Option<&str>) -> Result<(), ExtensionError> {
    let host = host_of(url).ok_or_else(|| {
        ExtensionError::Archive(format!("{url}: only https:// addresses are fetched"))
    })?;
    let ok = ALLOWED_HOSTS
        .iter()
        .any(|h| host == *h || host.ends_with(&format!(".{h}")))
        || extra_host.is_some_and(|e| e.eq_ignore_ascii_case(&host));
    if ok {
        Ok(())
    } else {
        Err(ExtensionError::Archive(format!(
            "{host} is not a registry Corvene downloads from"
        )))
    }
}

/// GET `url` as text (a registry's JSON), capped at [`MAX_JSON_BYTES`].
pub fn get_text(url: &str) -> Result<String, ExtensionError> {
    allowed(url, None)?;
    let mut response = agent()
        .get(url)
        .header("Accept", "application/json")
        .call()
        .map_err(|err| {
            ExtensionError::Archive(format!("{}: {err}", host_of(url).unwrap_or_default()))
        })?;
    let status = response.status().as_u16();
    if status != 200 {
        return Err(ExtensionError::Archive(format!(
            "{} answered with status {status}",
            host_of(url).unwrap_or_default()
        )));
    }
    let bytes = response
        .body_mut()
        .with_config()
        .limit(MAX_JSON_BYTES)
        .read_to_vec()
        .map_err(|err| ExtensionError::Archive(err.to_string()))?;
    String::from_utf8(bytes)
        .map_err(|_| ExtensionError::Archive("the answer is not UTF-8".to_string()))
}

/// What a download produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Downloaded {
    pub bytes: u64,
    pub sha256: String,
    /// the final URL after redirects
    pub url: String,
}

/// GET `url` into `dest` (written through a `.part` file), reporting
/// `(received, total)` as it goes; `cap` bounds the body.
pub fn download(
    url: &str,
    dest: &Path,
    cap: u64,
    extra_host: Option<&str>,
    progress: &mut dyn FnMut(u64, Option<u64>),
) -> Result<Downloaded, ExtensionError> {
    allowed(url, extra_host)?;
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut response = agent().get(url).call().map_err(|err| {
        ExtensionError::Archive(format!("{}: {err}", host_of(url).unwrap_or_default()))
    })?;
    let status = response.status().as_u16();
    if status != 200 {
        return Err(ExtensionError::Archive(format!(
            "{} answered with status {status}",
            host_of(url).unwrap_or_default()
        )));
    }
    let final_url = response.get_uri().to_string();
    // a redirect may land anywhere on HTTPS; the host rule applies to it too
    if final_url != url {
        allowed(&final_url, extra_host)?;
    }
    let total = response.body_mut().content_length();
    if total.is_some_and(|t| t > cap) {
        return Err(ExtensionError::Archive(format!(
            "the file is {} MB, more than Corvene accepts",
            total.unwrap_or(0) / (1024 * 1024)
        )));
    }
    let part = dest.with_extension("part");
    let mut file = std::fs::File::create(&part)?;
    let mut reader = response.body_mut().as_reader();
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    let mut received = 0u64;
    progress(0, total);
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        received += n as u64;
        if received > cap {
            drop(file);
            let _ = std::fs::remove_file(&part);
            return Err(ExtensionError::Archive(
                "the download is larger than Corvene accepts".to_string(),
            ));
        }
        hasher.update(&buf[..n]);
        file.write_all(&buf[..n])?;
        progress(received, total);
    }
    file.flush()?;
    drop(file);
    std::fs::rename(&part, dest)?;
    Ok(Downloaded {
        bytes: received,
        sha256: format!("{:x}", hasher.finalize()),
        url: final_url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_are_checked() {
        assert_eq!(
            host_of("https://Open-VSX.org/api/x"),
            Some("open-vsx.org".into())
        );
        assert_eq!(
            host_of("https://user@api.zed.dev:443/extensions"),
            Some("api.zed.dev".into())
        );
        assert_eq!(host_of("http://github.com/x"), None);
        assert!(allowed("https://codeload.github.com/a/b/tar.gz/HEAD", None).is_ok());
        assert!(allowed("https://evil.example/x", None).is_err());
        assert!(allowed("https://evil.example/x", Some("evil.example")).is_ok());
        assert!(allowed("https://notgithub.com/x", None).is_err());
        assert!(allowed("http://github.com/x", None).is_err());
    }
}
