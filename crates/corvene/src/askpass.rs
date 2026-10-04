//! `GIT_ASKPASS` mode: git runs this binary with the prompt as its only
//! argument (and `CORVENE_ASKPASS=1` in the environment) and reads the answer
//! from stdout. Usernames come from `CORVENE_ASKPASS_LOGINS`
//! (`host=login;host2=login2`), passwords from the macOS Keychain entry the
//! sign-in flow stored (`corvene_platform::keychain`). Anything unknown gets
//! an empty answer, which makes git fail with an authentication error that
//! the app turns into the "Authentication failed" dialog (GHD's
//! `askpass-trampoline` does the same over a local socket).

use std::collections::HashMap;

/// `Username for 'https://github.com': ` → `github.com`
/// `Password for 'https://octocat@github.com': ` → (`github.com`, `octocat`)
pub fn parse_prompt(prompt: &str) -> Option<(&'static str, String, Option<String>)> {
    let prompt = prompt.trim();
    let (kind, rest) = if let Some(rest) = prompt.strip_prefix("Username for '") {
        ("username", rest)
    } else {
        ("password", prompt.strip_prefix("Password for '")?)
    };
    let url = rest.split('\'').next()?;
    let without_scheme = url.split("://").nth(1).unwrap_or(url);
    let (user, host) = match without_scheme.split_once('@') {
        Some((user, host)) => (Some(user.to_string()), host),
        None => (None, without_scheme),
    };
    let host = host.split('/').next().unwrap_or(host).to_lowercase();
    Some((kind, host, user))
}

fn logins() -> HashMap<String, String> {
    parse_logins(&std::env::var("CORVENE_ASKPASS_LOGINS").unwrap_or_default())
}

/// `host=login;host2=login2`
pub fn parse_logins(logins: &str) -> HashMap<String, String> {
    logins
        .split(';')
        .filter_map(|pair| {
            let (host, login) = pair.split_once('=')?;
            Some((host.to_lowercase(), login.to_string()))
        })
        .collect()
}

/// Answer one prompt, or `None` when Corvene knows nothing about the host.
pub fn answer(prompt: &str) -> Option<String> {
    answer_with(prompt, logins())
}

/// [`answer`] with the host → login map given (Android: the helper process
/// passes its `CORVENE_ASKPASS_LOGINS` along with the prompt).
pub fn answer_with(prompt: &str, logins: HashMap<String, String>) -> Option<String> {
    // GHD `handleSSHHostAuthenticity`: ssh's unknown host question goes to the
    // AddSSHHost dialog, which Corvene does not have yet; declining makes ssh
    // refuse the host (`Host key verification failed.`)
    if let Some(info) = corvene_git::parse_add_ssh_host_prompt(prompt) {
        tracing::info!(
            host = %info.host,
            key_type = %info.key_type,
            "declined ssh's unknown host question"
        );
        return None;
    }
    let (kind, host, user) = parse_prompt(prompt)?;
    // git names the port (`host:8443`); logins and stored credentials are
    // kept by host name alone
    let host = match host.rsplit_once(':') {
        Some((name, port)) if !logins.contains_key(&host) && port.parse::<u16>().is_ok() => {
            name.to_string()
        }
        _ => host,
    };
    match kind {
        "username" => logins.get(&host).cloned(),
        _ => {
            let login = user.or_else(|| logins.get(&host).cloned())?;
            corvene_platform::keychain::token(&host, &login)
                .ok()
                .flatten()
                .or_else(|| {
                    corvene_platform::keychain::generic_password(&host, &login)
                        .ok()
                        .flatten()
                })
        }
    }
}

/// Entry point for `CORVENE_ASKPASS=1 corvene "<prompt>"`.
pub fn run() -> ! {
    let prompt = std::env::args().nth(1).unwrap_or_default();
    match answer(&prompt) {
        Some(value) => {
            println!("{value}");
            std::process::exit(0);
        }
        None => {
            println!();
            std::process::exit(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_git_prompts() {
        assert_eq!(
            parse_prompt("Username for 'https://github.com': "),
            Some(("username", "github.com".into(), None))
        );
        assert_eq!(
            parse_prompt("Password for 'https://octocat@GitHub.com': "),
            Some(("password", "github.com".into(), Some("octocat".into())))
        );
        assert_eq!(parse_prompt("Enter passphrase for key '/x': "), None);
    }

    #[test]
    fn declines_the_unknown_host_question() {
        let prompt = "The authenticity of host 'github.com (140.82.121.3)' can't be established.\n\
                      ED25519 key fingerprint is SHA256:+DiY3wvvV6TuJJhbpZisF/zLDA0zPMSvHdkr4UvCOqU.\n\
                      Are you sure you want to continue connecting (yes/no/[fingerprint])? ";
        assert_eq!(
            answer_with(prompt, parse_logins("github.com=octocat")),
            None
        );
    }

    #[test]
    fn a_port_in_the_prompt_does_not_hide_the_login() {
        let logins = parse_logins("ghe.corp=me");
        assert_eq!(
            answer_with("Username for 'https://ghe.corp:8443': ", logins).as_deref(),
            Some("me")
        );
    }
}
