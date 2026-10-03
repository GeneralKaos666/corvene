//! Port of GitHub Desktop's `app/test/unit/text-token-parser-test.ts`.
//!
//! GitHub Desktop's `Tokenizer` (`lib/text-token-parser.ts`) is
//! `corvene_core::text_tokens::tokenize(text, repository)`. The repository
//! GitHub Desktop's constructor keeps (`isRepositoryWithGitHubRepository` +
//! `getNonForkGitHubRepository`) is `TokenRepository::of(&repository)`.
//! `TokenResult` is `text_tokens::Token`: `TokenType.Text` is `Token::Text`,
//! `TokenType.Emoji` is `Token::Emoji`, `TokenType.Link` is `Token::Link`, and
//! `text` / `HyperlinkMatch.url` are their fields. The thin `Tokenizer` below
//! only keeps GitHub Desktop's call shape.
//!
//! GitHub Desktop hands the tokenizer an emoji map; Corvene's tokenizer looks
//! shortcodes up in its own table instead: the bundled Unicode emoji
//! (`emoji::all()`) and GitHub's image-only emoji (`emoji::custom()`, which
//! `Dispatcher::load_custom_emoji` fills from GitHub's `/emojis` API into
//! `emoji::CUSTOM`). The test's map holds one image-only emoji, `:shipit:`
//! (no `emoji` character), so `Tokenizer::new` puts it into `emoji::CUSTOM`
//! as a `CustomEmoji` whose `path` is the map's `url`, the same entry
//! `load_custom_emoji` would make. The bundled table stays; none of the
//! texts holds another shortcode (`:unknown:` is not one), so the results are
//! those of GitHub Desktop's one-entry map. Every test that writes the table
//! writes the same entry, and no other test depends on it being empty.
//!
//! `EmojiMatch.path`: Corvene's `Token::Emoji` carries no image path, because
//! GitHub's image-only emoji keep their shortcode as text in Corvene
//! (`.docs/deviations.md`); the two cases that check it are ignored.

use std::collections::HashMap;

use corvene_core::emoji::{CUSTOM, CustomEmoji};
use corvene_core::text_tokens::{Token, TokenRepository, tokenize};
use corvene_models::Repository;
use corvene_test_support::{GitHubRepoFixtureOptions, git_hub_repo_fixture};

use crate::text_tokens_support::{TokenType, kind, text, url};

/// GitHub Desktop's `Emoji` (`lib/emoji.ts`) without an `emoji` character,
/// i.e. one of GitHub's image-only emoji. `aliases` is not read by the
/// tokenizer and left out.
struct Emoji {
    url: &'static str,
}

/// The module's `emoji` map.
fn emoji() -> HashMap<&'static str, Emoji> {
    HashMap::from([(
        ":shipit:",
        Emoji {
            url: "/some/path.png",
        },
    )])
}

/// GitHub Desktop's `Tokenizer`: `new Tokenizer(emoji, repository?)` and
/// `tokenizer.tokenize(text)`, over `text_tokens::tokenize`.
struct Tokenizer {
    repository: Option<TokenRepository>,
}

impl Tokenizer {
    fn new(emoji: &HashMap<&'static str, Emoji>, repository: Option<&Repository>) -> Self {
        let mut custom = CUSTOM.write().unwrap();
        for (key, emoji) in emoji {
            let name = key.trim_matches(':');
            if !custom.iter().any(|c| c.name == name) {
                custom.push(CustomEmoji {
                    name: name.to_string(),
                    path: emoji.url.into(),
                });
            }
        }
        Self {
            repository: repository.and_then(TokenRepository::of),
        }
    }

    fn tokenize(&self, text: &str) -> Vec<Token> {
        tokenize(text, self.repository.as_ref())
    }
}

/// Stand-in for GitHub Desktop's `(token as EmojiMatch).path`, the image of
/// the emoji (`Emoji.url` from the map). `Token::Emoji` has no such field:
/// Corvene shows an image-only emoji's shortcode instead of its image.
fn path(_token: &Token) -> &str {
    unimplemented!("text_tokens::Token::Emoji carries no image path (GHD EmojiMatch.path)")
}

const HOST: &str = "https://github.com";
const LOGIN: &str = "shiftkey";
const NAME: &str = "some-repo";

/// The `with GitHub repository` describe's `htmlURL`.
fn html_url() -> String {
    format!("{HOST}/{LOGIN}/{NAME}")
}

/// The `with GitHub repository` describe's `repository`: `new
/// Repository('some/path/to/repo', 1, gitHubRepository, false)`.
fn repository() -> Repository {
    let mut repository = Repository::new(1, "some/path/to/repo");
    repository.github = Some(git_hub_repo_fixture(GitHubRepoFixtureOptions {
        name: NAME,
        owner: LOGIN,
        is_private: Some(false),
        ..Default::default()
    }));
    repository.missing = false;
    repository
}

// GHD: unit/text-token-parser-test.ts › Tokenizer › basic tests › preserves plain text string
#[test]
fn preserves_plain_text_string() {
    let text_ = "this is a string without anything interesting";
    let tokenizer = Tokenizer::new(&emoji(), None);
    let results = tokenizer.tokenize(text_);
    assert_eq!(results.len(), 1);
    assert_eq!(kind(&results[0]), TokenType::Text);
    assert_eq!(text(&results[0]), text_);
}

// GHD: unit/text-token-parser-test.ts › Tokenizer › basic tests › returns emoji between two string elements
#[test]
fn returns_emoji_between_two_string_elements() {
    let text_ = "let's :shipit: this thing";
    let tokenizer = Tokenizer::new(&emoji(), None);
    let results = tokenizer.tokenize(text_);
    assert_eq!(results.len(), 3);
    assert_eq!(kind(&results[0]), TokenType::Text);
    assert_eq!(text(&results[0]), "let's ");
    assert_eq!(kind(&results[1]), TokenType::Emoji);
    assert_eq!(text(&results[1]), ":shipit:");
    assert_eq!(kind(&results[2]), TokenType::Text);
    assert_eq!(text(&results[2]), " this thing");
}

// GHD: unit/text-token-parser-test.ts › Tokenizer › with GitHub repository › renders an emoji match
#[test]
#[ignore = "ghd: deviation: deviations.md 'GitHub's image-only emoji (:shipit:, :octocat:) in commit messages keep their shortcode as text': Token::Emoji has no path (GHD EmojiMatch.path)"]
fn github_renders_an_emoji_match() {
    let repository = repository();
    let text_ = "releasing the thing :shipit:";
    let tokenizer = Tokenizer::new(&emoji(), Some(&repository));
    let results = tokenizer.tokenize(text_);
    assert_eq!(results.len(), 2);
    assert_eq!(kind(&results[0]), TokenType::Text);
    assert_eq!(text(&results[0]), "releasing the thing ");

    assert_eq!(kind(&results[1]), TokenType::Emoji);
    let match_ = &results[1];

    assert_eq!(text(match_), ":shipit:");
    assert_eq!(path(match_), "/some/path.png");
}

// GHD: unit/text-token-parser-test.ts › Tokenizer › with GitHub repository › skips emoji when no match exists
#[test]
fn github_skips_emoji_when_no_match_exists() {
    let repository = repository();
    let text_ = "releasing the thing :unknown:";
    let tokenizer = Tokenizer::new(&emoji(), Some(&repository));
    let results = tokenizer.tokenize(text_);
    assert_eq!(results.len(), 1);
    assert_eq!(kind(&results[0]), TokenType::Text);
    assert_eq!(text(&results[0]), "releasing the thing :unknown:");
}

// GHD: unit/text-token-parser-test.ts › Tokenizer › with GitHub repository › does not render link when email address found
#[test]
fn does_not_render_link_when_email_address_found() {
    let repository = repository();
    let text_ = "the email address support@github.com should be ignored";
    let tokenizer = Tokenizer::new(&emoji(), Some(&repository));
    let results = tokenizer.tokenize(text_);
    assert_eq!(results.len(), 1);
    assert_eq!(kind(&results[0]), TokenType::Text);
    assert_eq!(
        text(&results[0]),
        "the email address support@github.com should be ignored"
    );
}

// GHD: unit/text-token-parser-test.ts › Tokenizer › with GitHub repository › render mention when text starts with a @
#[test]
fn render_mention_when_text_starts_with_an_at() {
    let repository = repository();
    let expected_uri = format!("{HOST}/{LOGIN}");
    let text_ = format!("@{LOGIN} was here");

    let tokenizer = Tokenizer::new(&emoji(), Some(&repository));
    let results = tokenizer.tokenize(&text_);
    assert_eq!(results.len(), 2);

    assert_eq!(kind(&results[0]), TokenType::Link);
    let mention = &results[0];

    assert_eq!(text(mention), "@shiftkey");
    assert_eq!(url(mention), expected_uri);

    assert_eq!(kind(&results[1]), TokenType::Text);
    assert_eq!(text(&results[1]), " was here");
}

// GHD: unit/text-token-parser-test.ts › Tokenizer › with GitHub repository › renders mention when token found
#[test]
fn renders_mention_when_token_found() {
    let repository = repository();
    let expected_uri = format!("{HOST}/{LOGIN}");
    let text_ = format!("fixed based on suggestion from @{LOGIN}");

    let tokenizer = Tokenizer::new(&emoji(), Some(&repository));
    let results = tokenizer.tokenize(&text_);
    assert_eq!(results.len(), 2);

    assert_eq!(kind(&results[0]), TokenType::Text);
    assert_eq!(text(&results[0]), "fixed based on suggestion from ");

    assert_eq!(kind(&results[1]), TokenType::Link);
    let mention = &results[1];

    assert_eq!(text(mention), "@shiftkey");
    assert_eq!(url(mention), expected_uri);
}

// GHD: unit/text-token-parser-test.ts › Tokenizer › with GitHub repository › ignores http prefix when no text after
#[test]
fn ignores_http_prefix_when_no_text_after() {
    let repository = repository();
    let text_ = "fix double http:// in avatar URLs";

    let tokenizer = Tokenizer::new(&emoji(), Some(&repository));
    let results = tokenizer.tokenize(text_);
    assert_eq!(results.len(), 1);

    assert_eq!(kind(&results[0]), TokenType::Text);
    assert_eq!(text(&results[0]), "fix double http:// in avatar URLs");
}

// GHD: unit/text-token-parser-test.ts › Tokenizer › with GitHub repository › ignores https prefix when no text after
#[test]
fn ignores_https_prefix_when_no_text_after() {
    let repository = repository();
    let text_ = "fix double https:// in avatar URLs";

    let tokenizer = Tokenizer::new(&emoji(), Some(&repository));
    let results = tokenizer.tokenize(text_);
    assert_eq!(results.len(), 1);

    assert_eq!(kind(&results[0]), TokenType::Text);
    assert_eq!(text(&results[0]), "fix double https:// in avatar URLs");
}

// GHD: unit/text-token-parser-test.ts › Tokenizer › with GitHub repository › renders link when an issue reference is found
#[test]
fn renders_link_when_an_issue_reference_is_found() {
    let repository = repository();
    let id = 955;
    let expected_uri = format!("{}/issues/{id}", html_url());
    let text_ = "Merge pull request #955 from desktop/computering-icons-for-all";

    let tokenizer = Tokenizer::new(&emoji(), Some(&repository));
    let results = tokenizer.tokenize(text_);
    assert_eq!(results.len(), 3);

    assert_eq!(kind(&results[0]), TokenType::Text);
    assert_eq!(text(&results[0]), "Merge pull request ");

    assert_eq!(kind(&results[1]), TokenType::Link);
    let mention = &results[1];

    assert_eq!(text(mention), "#955");
    assert_eq!(url(mention), expected_uri);

    assert_eq!(kind(&results[2]), TokenType::Text);
    assert_eq!(text(&results[2]), " from desktop/computering-icons-for-all");
}

// GHD: unit/text-token-parser-test.ts › Tokenizer › with GitHub repository › renders link when squash and merge
#[test]
fn renders_link_when_squash_and_merge() {
    let repository = repository();
    let id = 5203;
    let expected_uri = format!("{}/issues/{id}", html_url());
    let text_ = "Update README.md (#5203)";

    let tokenizer = Tokenizer::new(&emoji(), Some(&repository));
    let results = tokenizer.tokenize(text_);
    assert_eq!(results.len(), 3);

    assert_eq!(kind(&results[0]), TokenType::Text);
    assert_eq!(text(&results[0]), "Update README.md (");

    assert_eq!(kind(&results[1]), TokenType::Link);
    let mention = &results[1];

    assert_eq!(text(mention), "#5203");
    assert_eq!(url(mention), expected_uri);

    assert_eq!(kind(&results[2]), TokenType::Text);
    assert_eq!(text(&results[2]), ")");
}

// GHD: unit/text-token-parser-test.ts › Tokenizer › with GitHub repository › renders link and author mention when parsing release notes
#[test]
fn renders_link_and_author_mention_when_parsing_release_notes() {
    let repository = repository();
    let id = 5348;
    let expected_uri = format!("{}/issues/{id}", html_url());
    let text_ = "'Clone repository' menu item label is obscured on Windows - #5348. Thanks @Daniel-McCarthy!";

    let tokenizer = Tokenizer::new(&emoji(), Some(&repository));
    let results = tokenizer.tokenize(text_);
    assert_eq!(results.len(), 5);

    assert_eq!(kind(&results[0]), TokenType::Text);
    assert_eq!(
        text(&results[0]),
        "'Clone repository' menu item label is obscured on Windows - "
    );

    assert_eq!(kind(&results[1]), TokenType::Link);
    let issue_link = &results[1];

    assert_eq!(text(issue_link), "#5348");
    assert_eq!(url(issue_link), expected_uri);

    assert_eq!(kind(&results[2]), TokenType::Text);
    assert_eq!(text(&results[2]), ". Thanks ");

    assert_eq!(kind(&results[3]), TokenType::Link);
    let user_link = &results[3];

    assert_eq!(text(user_link), "@Daniel-McCarthy");
    assert_eq!(url(user_link), "https://github.com/Daniel-McCarthy");

    assert_eq!(kind(&results[4]), TokenType::Text);
    assert_eq!(text(&results[4]), "!");
}

// GHD: unit/text-token-parser-test.ts › Tokenizer › with GitHub repository › renders multiple issue links and mentions
#[test]
fn renders_multiple_issue_links_and_mentions() {
    let repository = repository();
    let first_id = 3174;
    let first_expected_url = format!("{}/issues/{first_id}", html_url());
    let second_id = 3184;
    let second_expected_url = format!("{}/issues/{second_id}", html_url());
    let third_id = 3207;
    let third_expected_url = format!("{}/issues/{third_id}", html_url());
    let text_ =
        "Assorted changelog typos - #3174 #3184 #3207. Thanks @strafe, @alanaasmaa and @jt2k!";

    let tokenizer = Tokenizer::new(&emoji(), Some(&repository));
    let results = tokenizer.tokenize(text_);
    assert_eq!(results.len(), 13);

    assert_eq!(kind(&results[0]), TokenType::Text);
    assert_eq!(text(&results[0]), "Assorted changelog typos - ");

    assert_eq!(kind(&results[1]), TokenType::Link);
    let first_issue_link = &results[1];

    assert_eq!(text(first_issue_link), "#3174");
    assert_eq!(url(first_issue_link), first_expected_url);

    assert_eq!(kind(&results[2]), TokenType::Text);
    assert_eq!(text(&results[2]), " ");

    assert_eq!(kind(&results[3]), TokenType::Link);
    let second_issue_link = &results[3];

    assert_eq!(text(second_issue_link), "#3184");
    assert_eq!(url(second_issue_link), second_expected_url);

    assert_eq!(kind(&results[4]), TokenType::Text);
    assert_eq!(text(&results[4]), " ");

    assert_eq!(kind(&results[5]), TokenType::Link);
    let third_issue_link = &results[5];

    assert_eq!(text(third_issue_link), "#3207");
    assert_eq!(url(third_issue_link), third_expected_url);

    assert_eq!(kind(&results[6]), TokenType::Text);
    assert_eq!(text(&results[6]), ". Thanks ");

    assert_eq!(kind(&results[7]), TokenType::Link);
    let first_user_link = &results[7];

    assert_eq!(text(first_user_link), "@strafe");
    assert_eq!(url(first_user_link), "https://github.com/strafe");

    assert_eq!(kind(&results[8]), TokenType::Text);
    assert_eq!(text(&results[8]), ", ");

    assert_eq!(kind(&results[9]), TokenType::Link);
    let second_user_link = &results[9];

    assert_eq!(text(second_user_link), "@alanaasmaa");
    assert_eq!(url(second_user_link), "https://github.com/alanaasmaa");

    assert_eq!(kind(&results[10]), TokenType::Text);
    assert_eq!(text(&results[10]), " and ");

    assert_eq!(kind(&results[11]), TokenType::Link);
    let third_user_link = &results[11];

    assert_eq!(text(third_user_link), "@jt2k");
    assert_eq!(url(third_user_link), "https://github.com/jt2k");

    assert_eq!(kind(&results[12]), TokenType::Text);
    assert_eq!(text(&results[12]), "!");
}

// GHD: unit/text-token-parser-test.ts › Tokenizer › with GitHub repository › renders multiple issue links and mentions even with commas
#[test]
fn renders_multiple_issue_links_and_mentions_even_with_commas() {
    let repository = repository();
    let first_id = 3174;
    let first_expected_url = format!("{}/issues/{first_id}", html_url());
    let second_id = 3184;
    let second_expected_url = format!("{}/issues/{second_id}", html_url());
    let third_id = 3207;
    let third_expected_url = format!("{}/issues/{third_id}", html_url());
    let text_ =
        "Assorted changelog typos - #3174, #3184 & #3207. Thanks @strafe, @alanaasmaa, and @jt2k!";

    let tokenizer = Tokenizer::new(&emoji(), Some(&repository));
    let results = tokenizer.tokenize(text_);
    assert_eq!(results.len(), 13);

    assert_eq!(kind(&results[0]), TokenType::Text);
    assert_eq!(text(&results[0]), "Assorted changelog typos - ");

    assert_eq!(kind(&results[1]), TokenType::Link);
    let first_issue_link = &results[1];

    assert_eq!(text(first_issue_link), "#3174");
    assert_eq!(url(first_issue_link), first_expected_url);

    assert_eq!(kind(&results[2]), TokenType::Text);
    assert_eq!(text(&results[2]), ", ");

    assert_eq!(kind(&results[3]), TokenType::Link);
    let second_issue_link = &results[3];

    assert_eq!(text(second_issue_link), "#3184");
    assert_eq!(url(second_issue_link), second_expected_url);

    assert_eq!(kind(&results[4]), TokenType::Text);
    assert_eq!(text(&results[4]), " & ");

    assert_eq!(kind(&results[5]), TokenType::Link);
    let third_issue_link = &results[5];

    assert_eq!(text(third_issue_link), "#3207");
    assert_eq!(url(third_issue_link), third_expected_url);

    assert_eq!(kind(&results[6]), TokenType::Text);
    assert_eq!(text(&results[6]), ". Thanks ");

    assert_eq!(kind(&results[7]), TokenType::Link);
    let first_user_link = &results[7];

    assert_eq!(text(first_user_link), "@strafe");
    assert_eq!(url(first_user_link), "https://github.com/strafe");

    assert_eq!(kind(&results[8]), TokenType::Text);
    assert_eq!(text(&results[8]), ", ");

    assert_eq!(kind(&results[9]), TokenType::Link);
    let second_user_link = &results[9];

    assert_eq!(text(second_user_link), "@alanaasmaa");
    assert_eq!(url(second_user_link), "https://github.com/alanaasmaa");

    assert_eq!(kind(&results[10]), TokenType::Text);
    assert_eq!(text(&results[10]), ", and ");

    assert_eq!(kind(&results[11]), TokenType::Link);
    let third_user_link = &results[11];

    assert_eq!(text(third_user_link), "@jt2k");
    assert_eq!(url(third_user_link), "https://github.com/jt2k");

    assert_eq!(kind(&results[12]), TokenType::Text);
    assert_eq!(text(&results[12]), "!");
}

/// The commit message of the two "full URL" cases.
const FULL_URL_TEXT: &str = r#"Note: we keep a "denylist" of authentication methods for which we do
not want to enable http.emptyAuth automatically. An allowlist would be
nicer, but less robust, as we want to support linking to several cURL
versions and the list of authentication methods (as well as their names)
changed over time.

[jes: actually added the "auto" handling, excluded Digest, too]

This fixes https://github.com/shiftkey/some-repo/issues/1034

Signed-off-by: Johannes Schindelin <johannes.schindelin@gmx.de>"#;

// GHD: unit/text-token-parser-test.ts › Tokenizer › with GitHub repository › converts full URL to issue shorthand
#[test]
fn converts_full_url_to_issue_shorthand() {
    let repository = repository();
    let text_ = FULL_URL_TEXT;

    let expected_before = r#"Note: we keep a "denylist" of authentication methods for which we do
not want to enable http.emptyAuth automatically. An allowlist would be
nicer, but less robust, as we want to support linking to several cURL
versions and the list of authentication methods (as well as their names)
changed over time.

[jes: actually added the "auto" handling, excluded Digest, too]

This fixes "#;

    let expected_after = r#"

Signed-off-by: Johannes Schindelin <johannes.schindelin@gmx.de>"#;

    let tokenizer = Tokenizer::new(&emoji(), Some(&repository));
    let results = tokenizer.tokenize(text_);

    assert_eq!(results.len(), 3);

    assert_eq!(kind(&results[0]), TokenType::Text);
    assert_eq!(text(&results[0]), expected_before);

    assert_eq!(kind(&results[1]), TokenType::Link);
    let issue = &results[1];

    assert_eq!(text(issue), "#1034");
    assert_eq!(
        url(issue),
        "https://github.com/shiftkey/some-repo/issues/1034"
    );

    assert_eq!(kind(&results[2]), TokenType::Text);
    assert_eq!(text(&results[2]), expected_after);
}

// GHD: unit/text-token-parser-test.ts › Tokenizer › with non-GitHub repository › renders an emoji match
#[test]
#[ignore = "ghd: deviation: deviations.md 'GitHub's image-only emoji (:shipit:, :octocat:) in commit messages keep their shortcode as text': Token::Emoji has no path (GHD EmojiMatch.path)"]
fn non_github_renders_an_emoji_match() {
    let text_ = "releasing the thing :shipit:";
    let tokenizer = Tokenizer::new(&emoji(), None);
    let results = tokenizer.tokenize(text_);
    assert_eq!(results.len(), 2);
    assert_eq!(kind(&results[0]), TokenType::Text);
    assert_eq!(text(&results[0]), "releasing the thing ");

    assert_eq!(kind(&results[1]), TokenType::Emoji);
    let match_ = &results[1];

    assert_eq!(text(match_), ":shipit:");
    assert_eq!(path(match_), "/some/path.png");
}

// GHD: unit/text-token-parser-test.ts › Tokenizer › with non-GitHub repository › skips emoji when no match exists
#[test]
fn non_github_skips_emoji_when_no_match_exists() {
    let text_ = "releasing the thing :unknown:";
    let tokenizer = Tokenizer::new(&emoji(), None);
    let results = tokenizer.tokenize(text_);
    assert_eq!(results.len(), 1);
    assert_eq!(kind(&results[0]), TokenType::Text);
    assert_eq!(text(&results[0]), text_);
}

// GHD: unit/text-token-parser-test.ts › Tokenizer › with non-GitHub repository › does not render link for mention
#[test]
fn does_not_render_link_for_mention() {
    let text_ = "fixed based on suggestion from @shiftkey";
    let tokenizer = Tokenizer::new(&emoji(), None);
    let results = tokenizer.tokenize(text_);
    assert_eq!(results.len(), 1);
    assert_eq!(kind(&results[0]), TokenType::Text);
    assert_eq!(text(&results[0]), text_);
}

// GHD: unit/text-token-parser-test.ts › Tokenizer › with non-GitHub repository › does not render link for issue reference
#[test]
fn does_not_render_link_for_issue_reference() {
    let text_ = "Merge pull request #955 from desktop/computering-icons-for-all";
    let tokenizer = Tokenizer::new(&emoji(), None);
    let results = tokenizer.tokenize(text_);
    assert_eq!(results.len(), 1);
    assert_eq!(kind(&results[0]), TokenType::Text);
    assert_eq!(text(&results[0]), text_);
}

// GHD: unit/text-token-parser-test.ts › Tokenizer › with non-GitHub repository › renders plain link for full URL
#[test]
fn renders_plain_link_for_full_url() {
    let text_ = FULL_URL_TEXT;

    let tokenizer = Tokenizer::new(&emoji(), None);
    let results = tokenizer.tokenize(text_);

    // other tests are looking at the newline formatting here
    // let's just verify the URL conversion works
    assert_eq!(results.len(), 3);

    assert_eq!(kind(&results[1]), TokenType::Link);
    let mention = &results[1];

    assert_eq!(
        text(mention),
        "https://github.com/shiftkey/some-repo/issues/1034"
    );
    assert_eq!(
        url(mention),
        "https://github.com/shiftkey/some-repo/issues/1034"
    );
}
