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

/// Corvene deviations from GHD's tokenizer; the default is GHD's behaviour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TokenOptions {
    /// `765-linkify-trailing-punctuation`
    pub trailing_punctuation: bool,
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
    fn trailing_punctuation() {
        let r = repo();
        let on = TokenOptions {
            trailing_punctuation: true,
        };
        assert_eq!(
            tokenize_with(
                "via https://x.io/pull/39177. (https://w.org/a_(b)) ok",
                None,
                on
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
            tokenize_with("[#12] #3: #4!?", Some(&r), on),
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
