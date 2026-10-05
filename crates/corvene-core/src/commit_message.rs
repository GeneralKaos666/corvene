//! Commit message helpers for the commit form.
//!
//! GHD: [`IDEAL_SUMMARY_LENGTH`] is `IdealSummaryLength`
//! (`app/src/lib/wrap-rich-text-commit-message.ts`), past which the summary
//! shows its length hint when Settings › Prompts › "Show commit length
//! warning" is on.
//!
//! Corvene additions (GHD has neither):
//! - `1301-conventional-commit-types`: [`CONVENTIONAL_TYPES`] and
//!   [`with_conventional_type`] behind the type menu next to the summary.
//! - `1302-wrap-commit-body`: [`wrap_body`] hard-wraps the description at
//!   [`BODY_WIDTH`] when committing.

/// GHD `IdealSummaryLength`.
pub const IDEAL_SUMMARY_LENGTH: usize = 50;

/// Corvene `1302-wrap-commit-body`: the column git's own guidance wraps
/// commit bodies at.
pub const BODY_WIDTH: usize = 72;

/// Corvene `1301-conventional-commit-types`: the types of the Conventional
/// Commits preset most tools use (`@commitlint/config-conventional`), with
/// what each is for.
pub const CONVENTIONAL_TYPES: &[(&str, &str)] = &[
    ("feat", "A new feature"),
    ("fix", "A bug fix"),
    ("docs", "Documentation only"),
    ("style", "Formatting, no code change"),
    ("refactor", "Neither a fix nor a feature"),
    ("perf", "A performance improvement"),
    ("test", "Adding or fixing tests"),
    ("build", "Build system or dependencies"),
    ("ci", "CI configuration"),
    ("chore", "Other changes"),
    ("revert", "Reverts a commit"),
];

/// A `type(scope)!: ` prefix at the start of `summary`: the type and the
/// byte length of the whole prefix (the space after the colon included).
pub fn conventional_prefix(summary: &str) -> Option<(&str, usize)> {
    let ty_len = summary
        .find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(summary.len());
    if ty_len == 0 {
        return None;
    }
    let mut rest = &summary[ty_len..];
    if let Some(after) = rest.strip_prefix('(') {
        let close = after.find(')')?;
        if after[..close].contains(['(', '\n']) {
            return None;
        }
        rest = &after[close + 1..];
    }
    rest = rest.strip_prefix('!').unwrap_or(rest);
    rest = rest.strip_prefix(':')?;
    rest = rest.strip_prefix(' ').unwrap_or(rest);
    Some((&summary[..ty_len], summary.len() - rest.len()))
}

/// The conventional type `summary` starts with, when it is one of
/// [`CONVENTIONAL_TYPES`].
pub fn conventional_type(summary: &str) -> Option<&'static str> {
    let (ty, _) = conventional_prefix(summary)?;
    CONVENTIONAL_TYPES
        .iter()
        .map(|(t, _)| *t)
        .find(|t| t.eq_ignore_ascii_case(ty))
}

/// `summary` with its type set to `ty` (`None` removes the whole prefix).
/// A known type is replaced keeping its scope and `!`; a summary without
/// one gets `ty: ` in front.
pub fn with_conventional_type(summary: &str, ty: Option<&str>) -> String {
    let known = conventional_prefix(summary).filter(|(t, _)| {
        CONVENTIONAL_TYPES
            .iter()
            .any(|(known, _)| known.eq_ignore_ascii_case(t))
    });
    match (known, ty) {
        (Some((old, _)), Some(ty)) => format!("{ty}{}", &summary[old.len()..]),
        (Some((_, len)), None) => summary[len..].to_string(),
        (None, Some(ty)) => format!("{ty}: {summary}"),
        (None, None) => summary.to_string(),
    }
}

/// Corvene `1302-wrap-commit-body`: `body` with every line longer than
/// `width` characters broken at spaces. Lines are never joined, so the
/// line breaks typed stay; list items and `>` quotes continue under their
/// text. Left alone: fenced (```` ``` ````, `~~~`) and indented code, table
/// rows, a closing paragraph of trailers (`Key: value`) and words longer
/// than the line (URLs).
pub fn wrap_body(body: &str, width: usize) -> String {
    let lines: Vec<&str> = body.split('\n').collect();
    let trailers_from = trailer_block_start(&lines);
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut fence: Option<&str> = None;
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if let Some(marker) = fence {
            if trimmed.starts_with(marker) {
                fence = None;
            }
            out.push(line.to_string());
            continue;
        }
        if let Some(marker) = ["```", "~~~"].into_iter().find(|m| trimmed.starts_with(m)) {
            fence = Some(marker);
            out.push(line.to_string());
            continue;
        }
        let verbatim = line.chars().count() <= width
            || line.starts_with('\t')
            || line.starts_with("    ")
            || trimmed.starts_with('|')
            || i >= trailers_from;
        if verbatim {
            out.push(line.to_string());
            continue;
        }
        wrap_line(line, width, &mut out);
    }
    out.join("\n")
}

/// Where the closing paragraph starts when every line of it is a
/// `Key: value` trailer (`lines.len()` when there is none).
fn trailer_block_start(lines: &[&str]) -> usize {
    let end = lines
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .map_or(0, |i| i + 1);
    let start = lines[..end]
        .iter()
        .rposition(|l| l.trim().is_empty())
        .map_or(0, |i| i + 1);
    // a body that is one paragraph has no trailer block
    if start == 0 || start == end {
        return lines.len();
    }
    let is_trailer = |line: &str| {
        line.split_once(": ").is_some_and(|(key, value)| {
            !key.is_empty()
                && !value.trim().is_empty()
                && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
    };
    if lines[start..end].iter().all(|l| is_trailer(l)) {
        start
    } else {
        lines.len()
    }
}

/// The part of `line` before its text: indentation plus a list marker
/// (`- `, `* `, `+ `, `1. `, `1) `) or `> `s, and the prefix its
/// continuation lines get.
fn line_lead(line: &str) -> (usize, String) {
    let indent = line.len() - line.trim_start_matches(' ').len();
    let rest = &line[indent..];
    let quotes = rest.len() - rest.trim_start_matches(['>', ' ']).len();
    if quotes > 0 && rest.starts_with('>') {
        let lead = &line[..indent + quotes];
        return (lead.len(), lead.to_string());
    }
    let marker = if ["- ", "* ", "+ "].iter().any(|m| rest.starts_with(m)) {
        2
    } else {
        let digits = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(0);
        if digits > 0 && (rest[digits..].starts_with(". ") || rest[digits..].starts_with(") ")) {
            digits + 2
        } else {
            0
        }
    };
    (indent + marker, " ".repeat(indent + marker))
}

fn wrap_line(line: &str, width: usize, out: &mut Vec<String>) {
    let (lead_len, continuation) = line_lead(line);
    let mut current = line[..lead_len].to_string();
    let mut current_len = current.chars().count();
    let mut has_word = false;
    for word in line[lead_len..].split(' ').filter(|w| !w.is_empty()) {
        let word_len = word.chars().count();
        if has_word && current_len + 1 + word_len > width {
            out.push(current);
            current = continuation.clone();
            current_len = current.chars().count();
            has_word = false;
        }
        if has_word {
            current.push(' ');
            current_len += 1;
        }
        current.push_str(word);
        current_len += word_len;
        has_word = true;
    }
    out.push(current);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixes_parse_with_scope_and_bang() {
        assert_eq!(conventional_prefix("feat: add x"), Some(("feat", 6)));
        assert_eq!(
            conventional_prefix("fix(parser)!: drop y"),
            Some(("fix", 14))
        );
        assert_eq!(conventional_prefix("feat:x"), Some(("feat", 5)));
        assert_eq!(conventional_prefix("Add parser"), None);
        assert_eq!(conventional_prefix("Fix it: now"), None);
        assert_eq!(conventional_type("Docs: readme"), Some("docs"));
        assert_eq!(conventional_type("note: readme"), None);
    }

    #[test]
    fn types_replace_keep_scope_or_prepend() {
        assert_eq!(with_conventional_type("Add x", Some("feat")), "feat: Add x");
        assert_eq!(
            with_conventional_type("fix(ui)!: Add x", Some("feat")),
            "feat(ui)!: Add x"
        );
        assert_eq!(with_conventional_type("fix(ui): Add x", None), "Add x");
        assert_eq!(with_conventional_type("", Some("docs")), "docs: ");
        assert_eq!(
            with_conventional_type("note: Add x", Some("fix")),
            "fix: note: Add x"
        );
    }

    #[test]
    fn long_lines_wrap_at_spaces_and_keep_breaks() {
        let body = "This sentence is long enough that it has to be wrapped somewhere past the seventy second column.\nShort line stays.";
        assert_eq!(
            wrap_body(body, 72),
            "This sentence is long enough that it has to be wrapped somewhere past\nthe seventy second column.\nShort line stays."
        );
    }

    #[test]
    fn lists_and_quotes_continue_under_their_text() {
        let body = "- first item that goes on and on and on until it is far too long for one line\n12. numbered item that goes on and on and on until it is far too long to fit\n> quoted text that goes on and on and on until it is far too long for a line";
        assert_eq!(
            wrap_body(body, 40),
            "- first item that goes on and on and on\n  until it is far too long for one line\n12. numbered item that goes on and on\n    and on until it is far too long to\n    fit\n> quoted text that goes on and on and on\n> until it is far too long for a line"
        );
    }

    #[test]
    fn code_tables_trailers_and_long_words_stay() {
        let url = format!("https://example.com/{}", "a".repeat(80));
        let code = format!("    {}", "x ".repeat(50));
        let fenced = format!("```\n{}\n```", "y ".repeat(50));
        let table = format!("| {} |", "cell ".repeat(20));
        let trailer = format!("Co-Authored-By: {} <a@b.c>", "Name ".repeat(15));
        let body = format!("See {url}\n{code}\n{fenced}\n{table}\n\n{trailer}");
        assert_eq!(
            wrap_body(&body, 72),
            format!("See\n{url}\n{code}\n{fenced}\n{table}\n\n{trailer}")
        );
    }

    #[test]
    fn a_lone_paragraph_is_not_a_trailer_block() {
        let line = format!("Note: {}", "word ".repeat(20).trim_end());
        assert!(wrap_body(&line, 72).contains('\n'));
    }
}
