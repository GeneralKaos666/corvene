//! Actions job logs in the app (Corvene addition, flag
//! `347-actions-job-logs`): a failed job or step in the check-run popover
//! opens the job's log in a dialog instead of the browser. GHD
//! (`ui/check-runs/ci-check-run-actions-job-step-list.tsx`) only links each
//! step to its page on GitHub, which stays as the fallback.
//!
//! The log is what `GET /repos/{o}/{r}/actions/jobs/{id}/logs` serves: the
//! job's steps' logs concatenated, every line prefixed with an ISO 8601
//! timestamp, workflow commands as `##[group]`, `##[endgroup]`, `##[error]`,
//! `##[warning]`, `##[notice]`, `##[command]`, `##[section]` and
//! `##[debug]` lines, and the tools' ANSI colours left in. [`parse_job_log`]
//! strips the escape sequences and the timestamps and classifies each line,
//! so the dialog can colour the markers, search the text and jump to the
//! failure. There is no step marker in the combined log: the failed step is
//! found by its `##[error]` line (the first one is where the run broke), or
//! by the `##[group]Run <step>` line that opens it.

use std::collections::HashMap;
use std::sync::Arc;

use corvene_models::{GitHubRepository, RefCheck};

use crate::dispatcher::Dispatcher;
use crate::host::Host;
use crate::remote::spawn_bg;

/// What a log line is, from its workflow command prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    Text,
    /// `##[group]`: a foldable section's title (a step's `Run …`).
    Group,
    /// `##[endgroup]`
    EndGroup,
    /// `##[error]`
    Error,
    /// `##[warning]`
    Warning,
    /// `##[notice]`
    Notice,
    /// `##[command]`: the command the runner executes.
    Command,
    /// `##[section]`
    Section,
    /// `##[debug]`
    Debug,
}

impl LineKind {
    /// The `##[…]` command and the rest of the line, when it starts with one.
    fn split(text: &str) -> (Self, &str) {
        let Some(rest) = text.strip_prefix("##[") else {
            return (Self::Text, text);
        };
        let Some((command, rest)) = rest.split_once(']') else {
            return (Self::Text, text);
        };
        let kind = match command {
            "group" => Self::Group,
            "endgroup" => Self::EndGroup,
            "error" => Self::Error,
            "warning" => Self::Warning,
            "notice" => Self::Notice,
            "command" => Self::Command,
            "section" => Self::Section,
            "debug" => Self::Debug,
            _ => return (Self::Text, text),
        };
        (kind, rest)
    }
}

/// One line of the log, cleaned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobLogLine {
    /// The line's timestamp (`2024-05-01T10:00:00.1234567Z`), when it had one.
    pub timestamp: Option<String>,
    pub kind: LineKind,
    /// The text after the timestamp and the workflow command, ANSI escapes
    /// removed.
    pub text: String,
}

/// A parsed job log.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct JobLog {
    pub lines: Vec<JobLogLine>,
    /// The `##[error]` lines, in order.
    pub errors: Vec<usize>,
    /// The `##[group]` lines, in order.
    pub groups: Vec<usize>,
}

impl JobLog {
    /// Where to scroll for `step` (a step's name as the API lists it, or
    /// `None` for the job's failure): the `##[group]Run <step>` line that
    /// opens the step when there is one, else the first `##[error]` line
    /// (the failed step ends with `Process completed with exit code N.`),
    /// else nothing.
    pub fn jump_target(&self, step: Option<&str>) -> Option<usize> {
        if let Some(step) = step
            && let Some(&line) = self.groups.iter().find(|&&ix| {
                let text = self.lines[ix].text.trim();
                text == step
                    || text
                        .strip_prefix("Run ")
                        .is_some_and(|run| run.trim() == step.trim())
            })
        {
            return Some(line);
        }
        self.errors.first().copied()
    }

    /// The cleaned text, one line per entry (what Copy puts on the clipboard).
    pub fn plain_text(&self) -> String {
        let mut out = String::new();
        for line in &self.lines {
            match line.kind {
                LineKind::Text => {}
                LineKind::Group => out.push_str("##[group]"),
                LineKind::EndGroup => out.push_str("##[endgroup]"),
                LineKind::Error => out.push_str("##[error]"),
                LineKind::Warning => out.push_str("##[warning]"),
                LineKind::Notice => out.push_str("##[notice]"),
                LineKind::Command => out.push_str("##[command]"),
                LineKind::Section => out.push_str("##[section]"),
                LineKind::Debug => out.push_str("##[debug]"),
            }
            out.push_str(&line.text);
            out.push('\n');
        }
        out
    }

    /// The lines whose text contains `query`, case-insensitively (empty
    /// query: none).
    pub fn search(&self, query: &str) -> Vec<usize> {
        if query.is_empty() {
            return Vec::new();
        }
        let query = query.to_lowercase();
        self.lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.text.to_lowercase().contains(&query))
            .map(|(ix, _)| ix)
            .collect()
    }
}

/// `text` without its ANSI escape sequences (CSI, OSC and the two-byte
/// escapes) and control characters other than tab.
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\u{1b}' => match chars.next() {
                // CSI: parameters, intermediates, then one final byte 0x40..=0x7e
                Some('[') => {
                    for c in chars.by_ref() {
                        if ('\u{40}'..='\u{7e}').contains(&c) {
                            break;
                        }
                    }
                }
                // OSC: up to BEL or ST (ESC \)
                Some(']') => {
                    let mut last = '\0';
                    for c in chars.by_ref() {
                        if c == '\u{7}' || (last == '\u{1b}' && c == '\\') {
                            break;
                        }
                        last = c;
                    }
                }
                // ESC ( x and the like
                Some('(') | Some(')') | Some('#') | Some('%') => {
                    chars.next();
                }
                _ => {}
            },
            '\r' => {}
            c if c.is_control() && c != '\t' => {}
            c => out.push(c),
        }
    }
    out
}

/// The leading `YYYY-MM-DDTHH:MM:SS[.fffffff]Z ` timestamp GitHub puts on
/// every line, split off.
fn split_timestamp(line: &str) -> (Option<String>, &str) {
    let bytes = line.as_bytes();
    // 2024-05-01T10:00:00
    if bytes.len() < 20
        || !bytes[..4].iter().all(u8::is_ascii_digit)
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
    {
        return (None, line);
    }
    let mut end = 19;
    if bytes.get(end) == Some(&b'.') {
        end += 1;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
    }
    if bytes.get(end) != Some(&b'Z') {
        return (None, line);
    }
    end += 1;
    let rest = line[end..].strip_prefix(' ').unwrap_or(&line[end..]);
    (Some(line[..end].to_string()), rest)
}

/// [`JobLog`] of the raw download.
pub fn parse_job_log(raw: &str) -> JobLog {
    let mut log = JobLog::default();
    for (ix, line) in raw.lines().enumerate() {
        let clean = strip_ansi(line);
        let (timestamp, rest) = split_timestamp(&clean);
        let (kind, text) = LineKind::split(rest);
        match kind {
            LineKind::Error => log.errors.push(ix),
            LineKind::Group => log.groups.push(ix),
            _ => {}
        }
        log.lines.push(JobLogLine {
            timestamp,
            kind,
            text: text.to_string(),
        });
    }
    // a trailing newline is not an empty last line
    log
}

/// A job log's state in `AppState::job_logs`, keyed by `(api base, job id)`.
#[derive(Clone, Debug)]
pub enum JobLogState {
    Loading,
    Loaded(Arc<JobLog>),
    /// The log could not be fetched (expired, no Actions job, network).
    Failed(String),
}

/// `job_logs` key.
pub fn job_log_key(gh: &GitHubRepository, job_id: u64) -> String {
    format!(
        "{}/repos/{}/{}/actions/jobs/{job_id}",
        gh.endpoint,
        gh.owner.to_lowercase(),
        gh.name.to_lowercase()
    )
}

/// The logs fetched this session.
#[derive(Default)]
pub struct JobLogStore {
    pub entries: HashMap<String, JobLogState>,
}

/// The message shown for a log that is not there.
pub const LOG_UNAVAILABLE: &str = "This log is no longer available. Logs expire after 90 days, and a check that is not a GitHub Actions job has none.";

impl Dispatcher {
    /// Open `check`'s log for `repo` (`347-actions-job-logs`): the dialog,
    /// and the fetch unless the log is already there. `step` scrolls to that
    /// step (the API's step name); `None` jumps to the failure.
    pub fn show_job_log(
        repo: u64,
        github: GitHubRepository,
        check: RefCheck,
        step: Option<String>,
        cx: &mut dyn Host,
    ) {
        Self::fetch_job_log(&github, check.id, false, cx);
        Self::show_popup(
            crate::state::Popup::ActionsJobLog {
                repo,
                github,
                check,
                step,
            },
            cx,
        );
    }

    /// Fetch `job_id`'s log into `AppState::job_logs` (`force`: again even
    /// when it is loaded, the dialog's refresh).
    pub fn fetch_job_log(github: &GitHubRepository, job_id: u64, force: bool, cx: &mut dyn Host) {
        let key = job_log_key(github, job_id);
        let start = Self::state(cx).update(cx, |s, cx| {
            match s.job_logs.entries.get(&key) {
                Some(JobLogState::Loading) => return false,
                Some(JobLogState::Loaded(_)) if !force => return false,
                _ => {}
            }
            s.job_logs.entries.insert(key.clone(), JobLogState::Loading);
            cx.notify();
            true
        });
        if !start {
            return;
        }
        let Some((endpoint, token, login)) = Self::api_for(github, cx) else {
            Self::state(cx).update(cx, |s, cx| {
                s.job_logs.entries.insert(
                    key,
                    JobLogState::Failed(
                        "Sign in to the repository's account to fetch logs.".into(),
                    ),
                );
                cx.notify();
            });
            return;
        };
        let (owner, name) = (github.owner.clone(), github.name.clone());
        let api_base = github.endpoint.clone();
        spawn_bg(
            cx,
            move || {
                let client = corvene_github::Client::new(endpoint, token);
                match client.job_logs(&owner, &name, job_id) {
                    Ok(Some(raw)) => (JobLogState::Loaded(Arc::new(parse_job_log(&raw))), false),
                    Ok(None) => (JobLogState::Failed(LOG_UNAVAILABLE.into()), false),
                    Err(err) => (
                        JobLogState::Failed(format!("The log could not be fetched. {err}")),
                        err.is_token_invalidated(),
                    ),
                }
            },
            move |(state, auth_failed), cx| {
                Self::state(cx).update(cx, |s, cx| {
                    s.job_logs.entries.insert(key, state);
                    cx.notify();
                });
                if auth_failed {
                    Self::token_invalidated(&api_base, &login, cx);
                }
            },
        );
    }

    /// Put a log in the store without fetching (dev hooks and tests).
    pub fn install_job_log(github: &GitHubRepository, job_id: u64, raw: &str, cx: &mut dyn Host) {
        let key = job_log_key(github, job_id);
        Self::state(cx).update(cx, |s, cx| {
            s.job_logs
                .entries
                .insert(key, JobLogState::Loaded(Arc::new(parse_job_log(raw))));
            cx.notify();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RAW: &str = "2024-05-01T10:00:00.0000001Z Requested labels: macos-15\n\
2024-05-01T10:00:01.0000000Z ##[group]Run cargo clippy\n\
2024-05-01T10:00:01.1000000Z \u{1b}[36;1mcargo clippy --all-targets\u{1b}[0m\n\
2024-05-01T10:00:02.0000000Z ##[endgroup]\n\
2024-05-01T10:00:03.0000000Z ##[group]Run cargo test --workspace\n\
2024-05-01T10:00:04.0000000Z \u{1b}[31mtest result: FAILED\u{1b}[0m. 1 failed\r\n\
2024-05-01T10:00:05.0000000Z ##[error]Process completed with exit code 101.\n\
2024-05-01T10:00:06.0000000Z ##[warning]Cache not saved\n\
plain line without a stamp\n";

    #[test]
    fn strips_escapes_and_stamps_and_classifies() {
        let log = parse_job_log(RAW);
        assert_eq!(log.lines.len(), 9);
        assert_eq!(
            log.lines[0].timestamp.as_deref(),
            Some("2024-05-01T10:00:00.0000001Z")
        );
        assert_eq!(log.lines[0].text, "Requested labels: macos-15");
        assert_eq!(log.lines[1].kind, LineKind::Group);
        assert_eq!(log.lines[1].text, "Run cargo clippy");
        assert_eq!(log.lines[2].text, "cargo clippy --all-targets");
        assert_eq!(log.lines[3].kind, LineKind::EndGroup);
        assert_eq!(log.lines[5].text, "test result: FAILED. 1 failed");
        assert_eq!(log.lines[6].kind, LineKind::Error);
        assert_eq!(log.lines[7].kind, LineKind::Warning);
        assert_eq!(log.lines[8].timestamp, None);
        assert_eq!(log.lines[8].text, "plain line without a stamp");
        assert_eq!(log.errors, vec![6]);
        assert_eq!(log.groups, vec![1, 4]);
    }

    #[test]
    fn jumps_to_the_step_group_or_the_first_error() {
        let log = parse_job_log(RAW);
        assert_eq!(log.jump_target(Some("cargo test --workspace")), Some(4));
        assert_eq!(log.jump_target(Some("Run cargo clippy")), Some(1));
        assert_eq!(log.jump_target(Some("Checkout")), Some(6));
        assert_eq!(log.jump_target(None), Some(6));
        assert_eq!(parse_job_log("ok\n").jump_target(None), None);
    }

    #[test]
    fn searches_case_insensitively() {
        let log = parse_job_log(RAW);
        assert_eq!(log.search("CARGO"), vec![1, 2, 4]);
        assert!(log.search("").is_empty());
    }

    #[test]
    fn strip_ansi_handles_osc_and_charset_escapes() {
        assert_eq!(
            strip_ansi("a\u{1b}]8;;https://x\u{7}link\u{1b}]8;;\u{7} b\u{1b}(Bc\u{1b}[1;32md"),
            "alink bcd"
        );
    }

    #[test]
    fn plain_text_keeps_the_markers() {
        let log = parse_job_log("2024-05-01T10:00:00Z ##[group]Run x\n2024-05-01T10:00:00Z y\n");
        assert_eq!(log.plain_text(), "##[group]Run x\ny\n");
    }
}
