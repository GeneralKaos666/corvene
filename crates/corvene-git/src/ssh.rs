//! ssh's prompts: GHD `parseAddSSHHostPrompt` (`lib/ssh/ssh.ts`).
//!
//! GHD points `SSH_ASKPASS` at its askpass trampoline, which shows the
//! `AddSSHHost` dialog for ssh's "The authenticity of host … can't be
//! established" question and answers `yes` or `no`. Corvene's askpass helper
//! (`crates/corvene/src/askpass.rs`) recognises the question with
//! [`parse_add_ssh_host_prompt`] but has no dialog to ask the user yet
//! (`.docs/TODO.md`), so it declines, and remote commands keep
//! `SSH_ASKPASS` unset (`crate::AskpassEnv`).

/// What ssh's unknown host question names (GHD's result of
/// `parseAddSSHHostPrompt`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AddSshHostInfo {
    pub host: String,
    pub ip: String,
    pub key_type: String,
    pub fingerprint: String,
}

/// GHD `parseAddSSHHostPrompt`: the host, address, key type and fingerprint
/// of ssh's unknown host question, `None` for any other prompt. Ported from
/// GHD's expression
///
/// ```text
/// ^The authenticity of host '([^ ]+) \(([^\)]+)\)' can't be established[^.]*\.\n([^ ]+) key fingerprint is ([^.]+)\.
/// ```
///
/// so it also reads "can't be established\nbut keys of different type are
/// already known for this host." and prompts with or without the
/// `[fingerprint]` answer.
pub fn parse_add_ssh_host_prompt(prompt: &str) -> Option<AddSshHostInfo> {
    let rest = prompt.strip_prefix("The authenticity of host '")?;
    let (host, rest) = rest.split_once(' ')?;
    let rest = rest.strip_prefix('(')?;
    let (ip, rest) = rest.split_once(')')?;
    let rest = rest.strip_prefix("' can't be established")?;
    // `[^.]*\.\n`
    let (_, rest) = rest.split_once('.')?;
    let rest = rest.strip_prefix('\n')?;
    let (key_type, rest) = rest.split_once(' ')?;
    let rest = rest.strip_prefix("key fingerprint is ")?;
    let (fingerprint, _) = rest.split_once('.')?;
    if host.is_empty() || ip.is_empty() || key_type.is_empty() || fingerprint.is_empty() {
        return None;
    }
    Some(AddSshHostInfo {
        host: host.to_string(),
        ip: ip.to_string(),
        key_type: key_type.to_string(),
        fingerprint: fingerprint.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn other_prompts_are_not_host_questions() {
        assert_eq!(
            parse_add_ssh_host_prompt("Enter passphrase for key '/Users/o/.ssh/id_ed25519': "),
            None
        );
        // `[^ ]+` stops at the first space: a host with none is no match
        assert_eq!(
            parse_add_ssh_host_prompt(
                "The authenticity of host 'x(1.2.3.4)' can't be established.\nRSA key fingerprint is A."
            ),
            None
        );
    }
}
