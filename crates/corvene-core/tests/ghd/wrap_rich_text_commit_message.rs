//! Port of GitHub Desktop's `app/test/unit/wrap-rich-text-commit-message-test.ts`.
//!
//! GitHub Desktop's `wrapRichTextCommitMessage(summary, body, tokenizer)`
//! (`lib/wrap-rich-text-commit-message.ts`) tokenizes a commit's summary and
//! body and moves whatever goes past 72 characters (`MaxSummaryLength`) of
//! the summary to the start of the body, with an ellipsis on both sides;
//! the History tab's expandable commit summary shows the result. Corvene
//! shows the whole summary (`corvene-ui/src/selected_commit.rs`) and has no
//! such function, so the cases call a stand-in and are ignored until it
//! exists.
//!
//! The tokenizer is Corvene's port of `Tokenizer`
//! (`corvene_core::text_tokens::tokenize`, whose repository is
//! `TokenRepository::of(repository)`); GitHub Desktop's `TokenResult` is
//! `text_tokens::Token` (`TokenType.Text` is `Token::Text`, `TokenType.Link`
//! is `Token::Link`, `HyperlinkMatch.url` its `url`). GitHub Desktop's
//! tokenizer gets no emoji here (`new Map()`); none of the texts has an
//! emoji shortcode.

use corvene_core::text_tokens::{Token, TokenRepository};
use corvene_models::{GitHubRepository, Repository};

/// GitHub Desktop's `TokenType`.
#[derive(Debug, PartialEq, Eq)]
enum TokenType {
    Emoji,
    Link,
    Text,
}

/// GitHub Desktop's `token.kind`.
fn kind(token: &Token) -> TokenType {
    match token {
        Token::Text(_) => TokenType::Text,
        Token::Emoji { .. } => TokenType::Emoji,
        Token::Link { .. } => TokenType::Link,
    }
}

/// GitHub Desktop's `token.text`.
fn text(token: &Token) -> &str {
    match token {
        Token::Text(text) | Token::Emoji { text, .. } | Token::Link { text, .. } => text,
    }
}

/// GitHub Desktop's `(token as HyperlinkMatch).url`.
fn url(token: &Token) -> &str {
    match token {
        Token::Link { url, .. } => url,
        other => panic!("not a HyperlinkMatch: {other:?}"),
    }
}

/// GitHub Desktop's `gitHubRepoFixture({ owner, name })`
/// (`test/helpers/github-repo-builder.ts`): a GitHub.com repository.
fn git_hub_repo_fixture(owner: &str, name: &str) -> GitHubRepository {
    let html_url = format!("https://github.com/{owner}/{name}");
    GitHubRepository {
        endpoint: "https://api.github.com".to_string(),
        owner: owner.to_string(),
        name: name.to_string(),
        clone_url: format!("{html_url}.git"),
        html_url,
        default_branch: None,
        private: false,
        fork: false,
        parent: None,
        archived: false,
        permissions: None,
        allow_forking: None,
    }
}

/// The describe's `tokenizer`: `new Tokenizer(new Map(), new Repository('.',
/// -1, gitHubRepoFixture({ owner: 'niik', name: 'commit-summary-wrap-tests'
/// }), false))`, i.e. the repository links point into.
fn tokenizer() -> Option<TokenRepository> {
    let mut repo = Repository::new(0, ".");
    repo.github = Some(git_hub_repo_fixture("niik", "commit-summary-wrap-tests"));
    TokenRepository::of(&repo)
}

/// Stand-in for GitHub Desktop's `wrapRichTextCommitMessage(summaryText,
/// bodyText, tokenizer)` with the default `maxSummaryLength` (72): the
/// summary tokens and the body tokens. Replace it with the Corvene function
/// once there is one (tokenizing with `text_tokens::tokenize(text,
/// tokenizer)`) and remove the `#[ignore]`s.
fn wrap_rich_text_commit_message(
    _summary_text: &str,
    _body_text: &str,
    _tokenizer: Option<&TokenRepository>,
) -> (Vec<Token>, Vec<Token>) {
    unimplemented!("Corvene has no wrapRichTextCommitMessage")
}

/// The describe's `wrap(summary, body = '')` helper.
fn wrap(summary: &str, body: &str) -> (Vec<Token>, Vec<Token>) {
    wrap_rich_text_commit_message(summary, body, tokenizer().as_ref())
}

// GHD: unit/wrap-rich-text-commit-message-test.ts › wrapRichTextCommitMessage › doesn't wrap at exactly 72 chars
#[test]
#[ignore = "ghd: missing: Corvene has no wrapRichTextCommitMessage (lib/wrap-rich-text-commit-message.ts); the commit summary is shown whole"]
fn doesnt_wrap_at_exactly_72_chars() {
    let summary_text = "weshouldnothardwrapthislongsummarywhichisexactly72charactersyeswetotally";
    let (summary, body) = wrap(summary_text, "");

    assert_eq!(summary.len(), 1);
    assert_eq!(body.len(), 0);

    assert_eq!(kind(&summary[0]), TokenType::Text);
    assert_eq!(text(&summary[0]), summary_text);
}

// GHD: unit/wrap-rich-text-commit-message-test.ts › wrapRichTextCommitMessage › hard wraps text longer than 72 chars
#[test]
#[ignore = "ghd: missing: Corvene has no wrapRichTextCommitMessage (lib/wrap-rich-text-commit-message.ts); the commit summary is shown whole"]
fn hard_wraps_text_longer_than_72_chars() {
    let summary_text =
        "weshouldabsolutelyhardwrapthislongsummarywhichexceeds72charactersyeswetotallyshould";
    let (summary, body) = wrap(summary_text, "");

    assert_eq!(summary.len(), 2);
    assert_eq!(body.len(), 2);

    assert_eq!(kind(&summary[0]), TokenType::Text);
    assert_eq!(text(&summary[0]), &summary_text[0..72]);
    assert_eq!(kind(&summary[1]), TokenType::Text);
    assert_eq!(text(&summary[1]), "…");

    assert_eq!(kind(&body[0]), TokenType::Text);
    assert_eq!(text(&body[0]), "…");
    assert_eq!(kind(&body[1]), TokenType::Text);
    assert_eq!(text(&body[1]), &summary_text[72..]);
}

// GHD: unit/wrap-rich-text-commit-message-test.ts › wrapRichTextCommitMessage › hard wraps text longer than 72 chars and joins it with the body
#[test]
#[ignore = "ghd: missing: Corvene has no wrapRichTextCommitMessage (lib/wrap-rich-text-commit-message.ts); the commit summary is shown whole"]
fn hard_wraps_text_longer_than_72_chars_and_joins_it_with_the_body() {
    let summary_text =
        "weshouldabsolutelyhardwrapthislongsummarywhichexceeds72charactersyeswetotallyshould";
    let body_text = "oh hi";
    let (summary, body) = wrap(summary_text, body_text);

    assert_eq!(summary.len(), 2);
    assert_eq!(body.len(), 4);

    assert_eq!(kind(&summary[0]), TokenType::Text);
    assert_eq!(text(&summary[0]), &summary_text[0..72]);
    assert_eq!(kind(&summary[1]), TokenType::Text);
    assert_eq!(text(&summary[1]), "…");

    assert_eq!(text(&body[0]), "…");
    assert_eq!(text(&body[1]), &summary_text[72..]);
    assert_eq!(text(&body[2]), "\n\n");
    assert_eq!(text(&body[3]), body_text);
}

// GHD: unit/wrap-rich-text-commit-message-test.ts › wrapRichTextCommitMessage › handles summaries which are exactly 72 chars after link shortening
#[test]
#[ignore = "ghd: missing: Corvene has no wrapRichTextCommitMessage (lib/wrap-rich-text-commit-message.ts); the commit summary is shown whole"]
fn handles_summaries_which_are_exactly_72_chars_after_link_shortening() {
    let summary_text = "This issue summary should be exactly 72 chars including the issue no: https://github.com/niik/commit-summary-wrap-tests/issues/1";
    let (summary, body) = wrap(summary_text, "");

    assert_eq!(summary.len(), 2);
    assert_eq!(body.len(), 0);

    assert_eq!(kind(&summary[0]), TokenType::Text);
    assert_eq!(
        text(&summary[0]),
        "This issue summary should be exactly 72 chars including the issue no: "
    );
    assert_eq!(kind(&summary[1]), TokenType::Link);
    assert_eq!(text(&summary[1]), "#1");
}

// GHD: unit/wrap-rich-text-commit-message-test.ts › wrapRichTextCommitMessage › takes issue link shortening into consideration
#[test]
#[ignore = "ghd: missing: Corvene has no wrapRichTextCommitMessage (lib/wrap-rich-text-commit-message.ts); the commit summary is shown whole"]
fn takes_issue_link_shortening_into_consideration() {
    let summary_text = "This issue link should be shortened to well under 72 characters: https://github.com/niik/commit-summary-wrap-tests/issues/1";
    let (summary, body) = wrap(summary_text, "");

    assert_eq!(summary.len(), 2);
    assert_eq!(body.len(), 0);

    assert_eq!(kind(&summary[0]), TokenType::Text);
    assert_eq!(
        text(&summary[0]),
        "This issue link should be shortened to well under 72 characters: "
    );
    assert_eq!(kind(&summary[1]), TokenType::Link);
    assert_eq!(text(&summary[1]), "#1");
    assert_eq!(
        url(&summary[1]),
        "https://github.com/niik/commit-summary-wrap-tests/issues/1"
    );
}

// GHD: unit/wrap-rich-text-commit-message-test.ts › wrapRichTextCommitMessage › handles multiple links
#[test]
#[ignore = "ghd: missing: Corvene has no wrapRichTextCommitMessage (lib/wrap-rich-text-commit-message.ts); the commit summary is shown whole"]
fn handles_multiple_links() {
    let summary_text = "Multiple links are fine https://github.com/niik/commit-summary-wrap-tests/issues/1 https://github.com/niik/commit-summary-wrap-tests/issues/2 https://github.com/niik/commit-summary-wrap-tests/issues/3 https://github.com/niik/commit-summary-wrap-tests/issues/4";
    let (summary, body) = wrap(summary_text, "");

    assert_eq!(summary.len(), 8);
    assert_eq!(body.len(), 0);

    let flattened: String = summary.iter().map(text).collect();
    assert_eq!(flattened, "Multiple links are fine #1 #2 #3 #4");
}

// GHD: unit/wrap-rich-text-commit-message-test.ts › wrapRichTextCommitMessage › wraps links properly
#[test]
#[ignore = "ghd: missing: Corvene has no wrapRichTextCommitMessage (lib/wrap-rich-text-commit-message.ts); the commit summary is shown whole"]
fn wraps_links_properly() {
    let summary_text = "Link should be truncated but open our release notes https://desktop.github.com/release-notes/";
    let (summary, body) = wrap(summary_text, "");

    assert_eq!(summary.len(), 3);
    assert_eq!(body.len(), 2);

    assert_eq!(kind(&summary[0]), TokenType::Text);
    assert_eq!(
        text(&summary[0]),
        "Link should be truncated but open our release notes "
    );

    assert_eq!(kind(&summary[1]), TokenType::Link);
    assert_eq!(text(&summary[1]), "https://desktop.gith");
    assert_eq!(
        url(&summary[1]),
        "https://desktop.github.com/release-notes/"
    );

    assert_eq!(kind(&summary[2]), TokenType::Text);
    assert_eq!(text(&summary[2]), "…");

    assert_eq!(kind(&body[0]), TokenType::Text);
    assert_eq!(text(&body[0]), "…");

    assert_eq!(kind(&body[1]), TokenType::Link);
    assert_eq!(text(&body[1]), "ub.com/release-notes/");
    assert_eq!(url(&body[1]), "https://desktop.github.com/release-notes/");
}
