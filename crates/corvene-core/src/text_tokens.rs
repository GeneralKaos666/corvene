//! Port of GHD `lib/text-token-parser.ts`: the look-ahead `Tokenizer` behind
//! `ui/lib/rich-text.tsx` that finds `:emoji:` shortcodes, `#123` issue
//! references, `@mentions` and `http(s)://` links in commit messages.
//!
//! In a GitHub repository (`getNonForkGitHubRepository`) all four are
//! recognised; elsewhere only emoji and links. A word runs to the next space
//! or newline, as in GHD, so `#1` inside `(#1)`, `#1.` and `#1,` is found but
//! `x#1` or `foo@bar` is not.
//!
//! Deviation: GitHub's image-only emoji (`:shipit:`) keep their shortcode as
//! text; GHD shows the image (`<img class="emoji">`).
//!
//! Deviation (`765-linkify-trailing-punctuation`, [`TokenOptions`]): a URL
//! may follow an opening bracket or quote and leaves out trailing
//! punctuation and unbalanced closing brackets, and an
//! issue reference any run of closing punctuation (`[#12]`, `#12:`), as
//! github.com does (GHD strips one `)`, `.` and `,` from issues only).
//!
//! Deviation (`766-cross-repository-issue-links`): `owner/repo#123` is one
//! link to that repository's issue and `owner/repo@<sha>` one link to its
//! commit (GHD links `#123` to the current repository and leaves the rest).
//!
//! Deviation (`341-custom-autolinks`, [`LinkRule`]): the repository's
//! GitHub autolinks (`TICKET-123` → the tracker's URL) also become links,
//! in GitHub and other repositories alike (GHD knows only GitHub's own
//! references); so do the references its `.issuetracker` file describes
//! (`1211-issuetracker-links`).
//!
//! [`wrap_rich_text_commit_message`] ports `lib/wrap-rich-text-commit-message.ts`:
//! a commit summary longer than 72 characters (counted on the tokens' shown
//! text, in UTF-16 units as JavaScript does) moves its overflow to the start
//! of the body. GHD can split a character made of two UTF-16 units; here
//! such a character moves whole.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use corvene_models::Repository;

/// GHD `TokenResult`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Token {
    /// `PlainText`
    Text(String),
    /// `EmojiMatch`: `text` is the shortcode, `emoji` the Unicode character
    /// (`None` for GitHub's image-only emoji).
    Emoji { text: String, emoji: Option<String> },
    /// `HyperlinkMatch`: `text` is shown, `url` opens on click.
    Link { text: String, url: String },
}

impl Token {
    /// The text this token shows (`RichText` renders the emoji character).
    pub fn display(&self) -> &str {
        match self {
            Token::Text(text) | Token::Link { text, .. } => text,
            Token::Emoji { text, emoji } => emoji.as_deref().unwrap_or(text),
        }
    }
}

/// The GitHub repository links point into: its `htmlURL` for issues and
/// `getHTMLURL(endpoint)` for mentions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenRepository {
    pub html_url: String,
    pub web_base: String,
}

impl TokenRepository {
    /// `getNonForkGitHubRepository(repository)`, when it has one.
    pub fn of(repository: &Repository) -> Option<Self> {
        let gh = repository.non_fork_github()?;
        Some(Self {
            html_url: gh.html_url.clone(),
            web_base: corvene_github::Endpoint::from_api_base(&gh.endpoint).web_base,
        })
    }
}

/// Corvene deviations from GHD's tokenizer; the default is GHD's behaviour.
#[derive(Clone, Debug, Default)]
pub struct TokenOptions {
    /// `765-linkify-trailing-punctuation`
    pub trailing_punctuation: bool,
    /// `766-cross-repository-issue-links`
    pub cross_repository: bool,
    /// The repository's own links ([`LinkRule`]), applied to the text
    /// between GHD's tokens.
    pub links: Option<Arc<[LinkRule]>>,
}

/// A repository's own kind of link in commit messages.
#[derive(Clone, Debug)]
pub enum LinkRule {
    /// `341-custom-autolinks`: a GitHub autolink (`GET
    /// repos/{owner}/{repo}/autolinks`, or one set in Repository Settings):
    /// `prefix` (any case) and a reference (digits; with `alphanumeric` also
    /// letters and `-`) at a word's start, linked to `url_template` with
    /// `<num>` replaced by the reference.
    Autolink {
        prefix: String,
        url_template: String,
        alphanumeric: bool,
    },
    /// A regular expression over the text; `url`'s `$1`, `$2`… (`$0`: the
    /// whole match) are the match's groups.
    Pattern { regex: regex::Regex, url: String },
}

impl LinkRule {
    /// The first match in `text` at or after byte `from`: its byte range
    /// and URL.
    fn find(&self, text: &str, from: usize) -> Option<(std::ops::Range<usize>, String)> {
        match self {
            LinkRule::Autolink {
                prefix,
                url_template,
                alphanumeric,
            } => {
                if prefix.is_empty() {
                    return None;
                }
                let lower = text.to_lowercase();
                // `to_lowercase` keeps byte offsets only for ASCII text
                if lower.len() != text.len() {
                    return None;
                }
                let wanted = prefix.to_lowercase();
                let reference = |b: u8| {
                    b.is_ascii_digit() || (*alphanumeric && (b.is_ascii_alphabetic() || b == b'-'))
                };
                let mut at = from;
                while let Some(found) = lower[at..].find(&wanted) {
                    let start = at + found;
                    let after = start + wanted.len();
                    at = start + 1;
                    let starts_word = text[..start]
                        .chars()
                        .next_back()
                        .is_none_or(|c| !c.is_alphanumeric() && c != '_');
                    if !starts_word {
                        continue;
                    }
                    let len = text.as_bytes()[after..]
                        .iter()
                        .take_while(|b| reference(**b))
                        .count();
                    // a reference ends a word, and does not end with `-`
                    let len = text[after..after + len].trim_end_matches('-').len();
                    let ends_word = text[after + len..]
                        .chars()
                        .next()
                        .is_none_or(|c| !c.is_alphanumeric() && c != '_');
                    if len == 0 || !ends_word {
                        continue;
                    }
                    let num = &text[after..after + len];
                    return Some((start..after + len, url_template.replace("<num>", num)));
                }
                None
            }
            LinkRule::Pattern { regex, url } => {
                let captures = regex.captures_at(text, from)?;
                let whole = captures.get(0)?;
                if whole.is_empty() {
                    return None;
                }
                let mut link = url.clone();
                // `$10` before `$1`
                for ix in (0..captures.len()).rev() {
                    let group = captures.get(ix).map_or("", |m| m.as_str());
                    link = link.replace(&format!("${ix}"), group);
                }
                Some((whole.range(), link))
            }
        }
    }
}

/// `341-custom-autolinks`: the text tokens of `tokens` with every match of
/// `rules` turned into a link (the earliest match first, the first rule on
/// a tie).
fn apply_link_rules(tokens: Vec<Token>, rules: &[LinkRule]) -> Vec<Token> {
    let mut out = Vec::with_capacity(tokens.len());
    for token in tokens {
        let Token::Text(text) = token else {
            out.push(token);
            continue;
        };
        let mut at = 0;
        let mut plain = 0;
        while at < text.len() {
            let next = rules
                .iter()
                .filter_map(|rule| rule.find(&text, at))
                .min_by_key(|(range, _)| range.start);
            let Some((range, url)) = next else {
                break;
            };
            if range.start > plain {
                out.push(Token::Text(text[plain..range.start].to_string()));
            }
            out.push(Token::Link {
                text: text[range.clone()].to_string(),
                url,
            });
            plain = range.end;
            at = range.end;
        }
        if plain < text.len() {
            out.push(Token::Text(text[plain..].to_string()));
        }
    }
    out
}

static EMOJI: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    crate::emoji::all()
        .iter()
        .map(|e| (e.key.as_str(), e.emoji.as_str()))
        .collect()
});

/// `Tokenizer.tokenize`
pub fn tokenize(text: &str, repository: Option<&TokenRepository>) -> Vec<Token> {
    tokenize_with(text, repository, TokenOptions::default())
}

/// [`tokenize`] with Corvene's [`TokenOptions`].
pub fn tokenize_with(
    text: &str,
    repository: Option<&TokenRepository>,
    options: TokenOptions,
) -> Vec<Token> {
    let links = options.links.clone();
    let mut t = Tokenizer {
        results: Vec::new(),
        current: String::new(),
        options,
    };
    let mut i = 0;
    while let Some(c) = text[i..].chars().next() {
        let matched = match c {
            ':' => t.scan_for_emoji(text, i),
            '#' => repository.and_then(|r| t.scan_for_issue(text, i, r)),
            '@' => repository.and_then(|r| {
                t.scan_for_cross_repository_commit(text, i, r)
                    .or_else(|| t.scan_for_mention(text, i, r))
            }),
            'h' => t.scan_for_hyperlink(text, i, repository),
            _ => None,
        };
        match matched {
            Some(next) => i = next,
            None => {
                t.current.push(c);
                i += c.len_utf8();
            }
        }
    }
    t.flush();
    match links {
        Some(rules) if !rules.is_empty() => apply_link_rules(t.results, &rules),
        _ => t.results,
    }
}

struct Tokenizer {
    results: Vec<Token>,
    current: String,
    options: TokenOptions,
}

impl Tokenizer {
    fn flush(&mut self) {
        if !self.current.is_empty() {
            self.results
                .push(Token::Text(std::mem::take(&mut self.current)));
        }
    }

    /// `getLastProcessedChar` is whitespace or absent (the text since the
    /// last token, so a token right after another one passes).
    fn after_whitespace(&self) -> bool {
        self.current
            .chars()
            .next_back()
            .is_none_or(char::is_whitespace)
    }

    fn scan_for_emoji(&mut self, text: &str, index: usize) -> Option<usize> {
        let next = end_of_word(text, index);
        let maybe = &text[index..next];
        if maybe.len() < 2 || !maybe.ends_with(':') {
            return None;
        }
        let emoji = match EMOJI.get(maybe) {
            Some(e) => Some((*e).to_string()),
            None => {
                let name = &maybe[1..maybe.len() - 1];
                crate::emoji::custom().iter().find(|c| c.name == name)?;
                None
            }
        };
        self.flush();
        self.results.push(Token::Emoji {
            text: maybe.to_string(),
            emoji,
        });
        Some(next)
    }

    fn scan_for_issue(
        &mut self,
        text: &str,
        index: usize,
        repository: &TokenRepository,
    ) -> Option<usize> {
        let mut next = end_of_word(text, index);
        if self.options.trailing_punctuation {
            // `765`: `[#123]`, `#123:`, `(#1, #2).` - any closing run
            next = index + text[index..next].trim_end_matches(ISSUE_TRAILING).len();
        } else {
            // `(#123)` from "squash and merge", `#123.` in release notes, and
            // lists of issues - one of each, in this order
            for suffix in [')', '.', ','] {
                if text[index..next].ends_with(suffix) {
                    next -= 1;
                }
            }
        }
        let digits = text[index..next].strip_prefix('#')?;
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        // `766`: `owner/repo` right before the `#` names another repository
        let other = self
            .options
            .cross_repository
            .then(|| self.repository_prefix())
            .flatten();
        let prefix = other.map(|len| self.current.split_off(self.current.len() - len));
        self.flush();
        // `parseInt` drops leading zeros
        let id = match digits.trim_start_matches('0') {
            "" => "0",
            id => id,
        };
        let (text, url) = match prefix {
            Some(prefix) => (
                format!("{prefix}{}", &text[index..next]),
                format!("{}/{prefix}/issues/{id}", repository.web_base),
            ),
            None => (
                text[index..next].to_string(),
                format!("{}/issues/{id}", repository.html_url),
            ),
        };
        self.results.push(Token::Link { text, url });
        Some(next)
    }

    /// `766-cross-repository-issue-links`: the byte length of an
    /// `owner/repo` at the end of the pending text that starts a word.
    fn repository_prefix(&self) -> Option<usize> {
        let tail_start = self
            .current
            .char_indices()
            .rev()
            .take_while(|(_, c)| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '/'))
            .last()
            .map(|(ix, _)| ix)?;
        let tail = &self.current[tail_start..];
        let before = self.current[..tail_start].chars().next_back();
        if before.is_some_and(|c| !c.is_whitespace() && !"([{<\"'".contains(c)) {
            return None;
        }
        let (owner, repo) = tail.split_once('/')?;
        let owner_ok = (1..=39).contains(&owner.len())
            && !owner.starts_with('-')
            && owner
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-');
        let repo_ok = !repo.is_empty()
            && !matches!(repo, "." | "..")
            && repo
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_'));
        (owner_ok && repo_ok).then_some(tail.len())
    }

    /// `766-cross-repository-issue-links`: `owner/repo@<sha>` (7 to 40 hex
    /// characters) as one link to that commit.
    fn scan_for_cross_repository_commit(
        &mut self,
        text: &str,
        index: usize,
        repository: &TokenRepository,
    ) -> Option<usize> {
        if !self.options.cross_repository {
            return None;
        }
        let len = self.repository_prefix()?;
        let end = end_of_word(text, index);
        let next = index + text[index..end].trim_end_matches(ISSUE_TRAILING).len();
        let sha = &text[index + 1..next];
        if !(7..=40).contains(&sha.len()) || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let prefix = self.current.split_off(self.current.len() - len);
        self.flush();
        self.results.push(Token::Link {
            text: format!("{prefix}@{sha}"),
            url: format!("{}/{prefix}/commit/{sha}", repository.web_base),
        });
        Some(next)
    }

    fn scan_for_mention(
        &mut self,
        text: &str,
        index: usize,
        repository: &TokenRepository,
    ) -> Option<usize> {
        // not part of an email address
        if !self.after_whitespace() {
            return None;
        }
        let mut next = end_of_word(text, index);
        // release notes end the last name with `!` or separate them with `,`
        if text[index..next].ends_with(['!', ',']) {
            next -= 1;
        }
        let name = text[index..next].strip_prefix('@')?;
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
            return None;
        }
        self.flush();
        self.results.push(Token::Link {
            text: text[index..next].to_string(),
            url: format!("{}/{name}", repository.web_base),
        });
        Some(next)
    }

    fn scan_for_hyperlink(
        &mut self,
        text: &str,
        index: usize,
        repository: Option<&TokenRepository>,
    ) -> Option<usize> {
        // not the middle of a word (`765`: an opening bracket or quote may
        // come before it, `(https://…)`)
        let after_opening = self.options.trailing_punctuation
            && self.current.ends_with(['(', '[', '{', '<', '"', '\'']);
        if !self.after_whitespace() && !after_opening {
            return None;
        }
        let mut next = end_of_word(text, index);
        if self.options.trailing_punctuation {
            next = index + trim_url_end(&text[index..next]).len();
        }
        let maybe = &text[index..next];
        let rest = maybe
            .strip_prefix("https://")
            .or_else(|| maybe.strip_prefix("http://"))?;
        if rest.is_empty() {
            return None;
        }
        self.flush();
        // a link to one of the repository's issues shows as `#123`
        let issue = repository
            .filter(|r| !r.html_url.is_empty())
            .filter(|r| {
                maybe
                    .to_lowercase()
                    .starts_with(&format!("{}/issues/", r.html_url.to_lowercase()))
            })
            .and_then(|_| {
                let at = maybe.find("/issues/")? + "/issues/".len();
                let digits: &str = &maybe[at..];
                let end = digits
                    .find(|c: char| !c.is_ascii_digit())
                    .unwrap_or(digits.len());
                (end > 0).then(|| format!("#{}", &digits[..end]))
            });
        self.results.push(Token::Link {
            text: issue.unwrap_or_else(|| maybe.to_string()),
            url: maybe.to_string(),
        });
        Some(next)
    }
}

/// `765-linkify-trailing-punctuation`: what may follow an issue number.
const ISSUE_TRAILING: &[char] = &[')', ']', '}', '>', '.', ',', ';', ':', '!', '?', '\'', '"'];

/// `765-linkify-trailing-punctuation`: `url` without the trailing punctuation
/// github.com leaves out of an autolink, and without closing brackets that
/// do not close one opened inside it (`(see https://x.io/a_(b))` keeps one).
pub fn trim_url_end(url: &str) -> &str {
    let mut url = url;
    loop {
        let Some(last) = url.chars().next_back() else {
            return url;
        };
        let unbalanced = |open: char| url.matches(open).count() < url.matches(last).count();
        let strip = match last {
            '.' | ',' | ';' | ':' | '!' | '?' | '\'' | '"' | '*' | '_' | '~' => true,
            ')' => unbalanced('('),
            ']' => unbalanced('['),
            '}' => unbalanced('{'),
            '>' => true,
            _ => false,
        };
        if !strip {
            return url;
        }
        url = &url[..url.len() - last.len_utf8()];
    }
}

/// GHD `MaxSummaryLength`: the summary width
/// [`wrap_rich_text_commit_message`] wraps at.
pub const MAX_SUMMARY_LENGTH: usize = 72;

/// GHD `wrapRichTextCommitMessage(summaryText, bodyText, tokenizer)`
/// (`lib/wrap-rich-text-commit-message.ts`): the summary and body tokens of
/// a commit message, the summary (right-trimmed) cut after
/// [`MAX_SUMMARY_LENGTH`] characters of shown text. What does not fit moves
/// to the start of the body between `…` (the summary ends with one too),
/// followed by a blank line and the body. Text is cut anywhere, an emoji
/// (counted as two characters) or an issue link moves whole, and another
/// link is cut when more than five characters of it fit.
pub fn wrap_rich_text_commit_message(
    summary_text: &str,
    body_text: &str,
    repository: Option<&TokenRepository>,
) -> (Vec<Token>, Vec<Token>) {
    wrap_rich_text_commit_message_with(summary_text, body_text, repository, TokenOptions::default())
}

/// [`wrap_rich_text_commit_message`] with Corvene's [`TokenOptions`].
pub fn wrap_rich_text_commit_message_with(
    summary_text: &str,
    body_text: &str,
    repository: Option<&TokenRepository>,
    options: TokenOptions,
) -> (Vec<Token>, Vec<Token>) {
    let mut summary = Vec::new();
    let mut overflow = Vec::new();
    let mut remainder = MAX_SUMMARY_LENGTH;
    for token in tokenize_with(summary_text.trim_end(), repository, options.clone()) {
        // an emoji shows about as wide as two characters
        let char_count = match &token {
            Token::Emoji { .. } => 2,
            Token::Text(text) | Token::Link { text, .. } => utf16_len(text),
        };
        if remainder == 0 {
            overflow.push(token);
        } else if remainder >= char_count {
            remainder -= char_count;
            summary.push(token);
        } else {
            match token {
                // text is always hard-wrapped
                Token::Text(text) => {
                    let (head, tail) = split_utf16(&text, remainder);
                    summary.push(Token::Text(head.to_string()));
                    overflow.push(Token::Text(tail.to_string()));
                }
                // links show at least a few characters (no `h` or `@`), and an
                // issue link is never cut; GHD's cut halves link to the text
                Token::Link { text, .. } if !text.starts_with('#') && remainder > 5 => {
                    let (head, tail) = split_utf16(&text, remainder);
                    summary.push(Token::Link {
                        text: head.to_string(),
                        url: text.clone(),
                    });
                    overflow.push(Token::Link {
                        text: tail.to_string(),
                        url: text.clone(),
                    });
                }
                token => overflow.push(token),
            }
            remainder = 0;
        }
    }
    let mut body = tokenize_with(body_text.trim_end(), repository, options);
    if !overflow.is_empty() {
        let ellipsis = || Token::Text("…".to_string());
        summary.push(ellipsis());
        let mut moved = vec![ellipsis()];
        moved.extend(overflow);
        if !body.is_empty() {
            moved.push(Token::Text("\n\n".to_string()));
            moved.append(&mut body);
        }
        body = moved;
    }
    (summary, body)
}

/// JavaScript's `text.length`.
fn utf16_len(text: &str) -> usize {
    text.chars().map(char::len_utf16).sum()
}

/// JavaScript's `substring(0, at)` / `substring(at)`, cut before the char
/// that would cross `at` UTF-16 units.
fn split_utf16(text: &str, at: usize) -> (&str, &str) {
    let mut units = 0;
    for (ix, c) in text.char_indices() {
        units += c.len_utf16();
        if units > at {
            return text.split_at(ix);
        }
    }
    (text, "")
}

/// `RichText` without a repository and `renderUrlsAsLinks={false}` (commit
/// list rows, Undo Commit): the text with its emoji shortcodes replaced.
pub fn with_emoji(text: &str) -> String {
    if !text.contains(':') {
        return text.to_string();
    }
    tokenize(text, None).iter().map(Token::display).collect()
}

/// `scanForEndOfWord`: the next space or newline after `index`, or the end.
fn end_of_word(text: &str, index: usize) -> usize {
    text[index + 1..]
        .find([' ', '\n'])
        .map_or(text.len(), |ix| index + 1 + ix)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> TokenRepository {
        TokenRepository {
            html_url: "https://github.com/o/r".into(),
            web_base: "https://github.com".into(),
        }
    }

    fn link(text: &str, url: &str) -> Token {
        Token::Link {
            text: text.into(),
            url: url.into(),
        }
    }

    fn text(t: &str) -> Token {
        Token::Text(t.into())
    }

    fn with_links(rules: Vec<LinkRule>) -> TokenOptions {
        TokenOptions {
            links: Some(rules.into()),
            ..TokenOptions::default()
        }
    }

    #[test]
    fn autolinks_link_prefixed_references_at_word_starts() {
        let options = with_links(vec![
            LinkRule::Autolink {
                prefix: "TICKET-".into(),
                url_template: "https://t.example/?id=<num>".into(),
                alphanumeric: false,
            },
            LinkRule::Autolink {
                prefix: "JIRA-".into(),
                url_template: "https://j.example/browse/JIRA-<num>".into(),
                alphanumeric: true,
            },
        ]);
        assert_eq!(
            tokenize_with(
                "Fix ticket-12 and (JIRA-ab-3), not xTICKET-1 or TICKET-1a or TICKET-",
                None,
                options.clone()
            ),
            vec![
                text("Fix "),
                link("ticket-12", "https://t.example/?id=12"),
                text(" and ("),
                link("JIRA-ab-3", "https://j.example/browse/JIRA-ab-3"),
                text("), not xTICKET-1 or TICKET-1a or TICKET-"),
            ]
        );
        // GHD's own tokens stay as they are
        assert_eq!(
            tokenize_with("see #4 TICKET-5", Some(&repo()), options),
            vec![
                text("see "),
                link("#4", "https://github.com/o/r/issues/4"),
                text(" "),
                link("TICKET-5", "https://t.example/?id=5"),
            ]
        );
    }

    #[test]
    fn patterns_fill_in_their_groups() {
        let options = with_links(vec![LinkRule::Pattern {
            regex: regex::Regex::new(r"\b([A-Z]+)-(\d+)\b").unwrap(),
            url: "https://tracker.example/$1/issue/$2".into(),
        }]);
        assert_eq!(
            tokenize_with("Closes ABC-7 and DEF-12.", None, options),
            vec![
                text("Closes "),
                link("ABC-7", "https://tracker.example/ABC/issue/7"),
                text(" and "),
                link("DEF-12", "https://tracker.example/DEF/issue/12"),
                text("."),
            ]
        );
    }

    #[test]
    fn issues() {
        let r = repo();
        assert_eq!(
            tokenize("Closes #452", Some(&r)),
            [
                text("Closes "),
                link("#452", "https://github.com/o/r/issues/452")
            ]
        );
        assert_eq!(
            tokenize("Fix (#12)\n#3, #4.", Some(&r)),
            [
                text("Fix ("),
                link("#12", "https://github.com/o/r/issues/12"),
                text(")\n"),
                link("#3", "https://github.com/o/r/issues/3"),
                text(", "),
                link("#4", "https://github.com/o/r/issues/4"),
                text("."),
            ]
        );
        // mid-word `#` still counts (GHD does not check the previous char)
        assert_eq!(
            tokenize("x#7 #7a #", Some(&r)),
            [
                text("x"),
                link("#7", "https://github.com/o/r/issues/7"),
                text(" #7a #"),
            ]
        );
        // not a GitHub repository: plain text
        assert_eq!(tokenize("Closes #452", None), [text("Closes #452")]);
    }

    #[test]
    fn mentions() {
        let r = repo();
        assert_eq!(
            tokenize("thanks @a-b, @c! me@x.com", Some(&r)),
            [
                text("thanks "),
                link("@a-b", "https://github.com/a-b"),
                text(", "),
                link("@c", "https://github.com/c"),
                text("! me@x.com"),
            ]
        );
    }

    #[test]
    fn hyperlinks() {
        let r = repo();
        assert_eq!(
            tokenize("see https://x.io/a. and xhttp://y", None),
            [
                text("see "),
                link("https://x.io/a.", "https://x.io/a."),
                text(" and xhttp://y"),
            ]
        );
        assert_eq!(
            tokenize("https://GitHub.com/o/r/issues/9#c", Some(&r)),
            [link("#9", "https://GitHub.com/o/r/issues/9#c")]
        );
        assert_eq!(tokenize("http:// h", None), [text("http:// h")]);
    }

    #[test]
    fn trailing_punctuation() {
        let r = repo();
        let on = TokenOptions {
            trailing_punctuation: true,
            ..TokenOptions::default()
        };
        assert_eq!(
            tokenize_with(
                "via https://x.io/pull/39177. (https://w.org/a_(b)) ok",
                None,
                on.clone()
            ),
            [
                text("via "),
                link("https://x.io/pull/39177", "https://x.io/pull/39177"),
                text(". ("),
                link("https://w.org/a_(b)", "https://w.org/a_(b)"),
                text(") ok"),
            ]
        );
        assert_eq!(
            tokenize_with("[#12] #3: #4!?", Some(&r), on.clone()),
            [
                text("["),
                link("#12", "https://github.com/o/r/issues/12"),
                text("] "),
                link("#3", "https://github.com/o/r/issues/3"),
                text(": "),
                link("#4", "https://github.com/o/r/issues/4"),
                text("!?"),
            ]
        );
        // GHD: the bracket and colon break the reference
        assert_eq!(tokenize("[#12] #3:", Some(&r)), [text("[#12] #3:")]);
        assert_eq!(tokenize_with("https://.", None, on), [text("https://.")]);
    }

    #[test]
    fn cross_repository_references() {
        let r = repo();
        let on = TokenOptions {
            cross_repository: true,
            ..TokenOptions::default()
        };
        assert_eq!(
            tokenize_with(
                "See a-b/c.d#12, (x/y@a5c37851) and a/b/c#3 #4",
                Some(&r),
                on.clone()
            ),
            [
                text("See "),
                link("a-b/c.d#12", "https://github.com/a-b/c.d/issues/12"),
                text(", ("),
                link("x/y@a5c37851", "https://github.com/x/y/commit/a5c37851"),
                text(") and a/b/c"),
                link("#3", "https://github.com/o/r/issues/3"),
                text(" "),
                link("#4", "https://github.com/o/r/issues/4"),
            ]
        );
        // a short or non-hex word after `@` is no commit; mid-word stays text
        assert_eq!(
            tokenize_with("x/y@abc x/y@zzzzzzzz", Some(&r), on),
            [text("x/y@abc x/y@zzzzzzzz")]
        );
        // GHD: only `#12` is linked, to the current repository
        assert_eq!(
            tokenize("a/b#12", Some(&r)),
            [text("a/b"), link("#12", "https://github.com/o/r/issues/12")]
        );
    }

    #[test]
    fn wraps_long_summaries_into_the_body() {
        let summary = "a".repeat(69) + " :tada: more";
        let (title, body) = wrap_rich_text_commit_message(&summary, "body", None);
        // the emoji counts as two characters and still fits; the rest moves
        assert_eq!(title.last(), Some(&text("…")));
        assert_eq!(body.first(), Some(&text("…")));
        assert_eq!(body[1], text(" more"));
        assert_eq!(body[2], text("\n\n"));
        assert_eq!(body[3], text("body"));
        // a char of two UTF-16 units is not cut
        assert_eq!(split_utf16("a🎉b", 2), ("a", "🎉b"));
        assert_eq!(split_utf16("a🎉b", 3), ("a🎉", "b"));
        let (title, body) = wrap_rich_text_commit_message("short  ", "", None);
        assert_eq!((title, body), (vec![text("short")], Vec::new()));
    }

    #[test]
    fn emoji() {
        assert_eq!(
            tokenize("ship :tada: it :nope: :", None),
            [
                text("ship "),
                Token::Emoji {
                    text: ":tada:".into(),
                    emoji: Some("🎉".into())
                },
                text(" it :nope: :"),
            ]
        );
    }
}
