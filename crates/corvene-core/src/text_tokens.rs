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
//! [`wrap_rich_text_commit_message`] ports `lib/wrap-rich-text-commit-message.ts`:
//! a commit summary longer than 72 characters (counted on the tokens' shown
//! text, in UTF-16 units as JavaScript does) moves its overflow to the start
//! of the body. GHD can split a character made of two UTF-16 units; here
//! such a character moves whole.

use std::collections::HashMap;
use std::sync::LazyLock;

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

static EMOJI: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    crate::emoji::all()
        .iter()
        .map(|e| (e.key.as_str(), e.emoji.as_str()))
        .collect()
});

/// `Tokenizer.tokenize`
pub fn tokenize(text: &str, repository: Option<&TokenRepository>) -> Vec<Token> {
    let mut t = Tokenizer {
        results: Vec::new(),
        current: String::new(),
    };
    let mut i = 0;
    while let Some(c) = text[i..].chars().next() {
        let matched = match c {
            ':' => t.scan_for_emoji(text, i),
            '#' => repository.and_then(|r| t.scan_for_issue(text, i, r)),
            '@' => repository.and_then(|r| t.scan_for_mention(text, i, r)),
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
    t.results
}

struct Tokenizer {
    results: Vec<Token>,
    current: String,
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
        // `(#123)` from "squash and merge", `#123.` in release notes, and
        // lists of issues - one of each, in this order
        for suffix in [')', '.', ','] {
            if text[index..next].ends_with(suffix) {
                next -= 1;
            }
        }
        let digits = text[index..next].strip_prefix('#')?;
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        self.flush();
        // `parseInt` drops leading zeros
        let id = match digits.trim_start_matches('0') {
            "" => "0",
            id => id,
        };
        self.results.push(Token::Link {
            text: text[index..next].to_string(),
            url: format!("{}/issues/{id}", repository.html_url),
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
        // not the middle of a word
        if !self.after_whitespace() {
            return None;
        }
        let next = end_of_word(text, index);
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
    let mut summary = Vec::new();
    let mut overflow = Vec::new();
    let mut remainder = MAX_SUMMARY_LENGTH;
    for token in tokenize(summary_text.trim_end(), repository) {
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
    let mut body = tokenize(body_text.trim_end(), repository);
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
