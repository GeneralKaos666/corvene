//! Port of GitHub Desktop's `app/test/unit/stores/github-user-store-test.ts`.
//!
//! GitHub Desktop's `matchMentionableUsers(users, query, maxHits =
//! DefaultMaxHits)` (`lib/stores/github-user-store.ts`) is Corvene's
//! `corvene_core::users_matching(users, text, exclude_login, max_hits)`
//! with `exclude_login: None` (the extra parameter is the autocompletion
//! provider's own-handle filter, which `matchMentionableUsers` does not
//! apply) and `DEFAULT_MAX_HITS` as the default. `IMentionableUser` is
//! `corvene_core::MentionableUser`, whose `email` and `avatar_url` are
//! optional (always set here, as in GitHub Desktop's type).

use corvene_core::{DEFAULT_MAX_HITS, MentionableUser, users_matching};

/// `matchMentionableUsers(users, query, maxHits = DefaultMaxHits)`.
fn match_mentionable_users(
    users: &[MentionableUser],
    query: &str,
    max_hits: Option<usize>,
) -> Vec<MentionableUser> {
    users_matching(users, query, None, max_hits.unwrap_or(DEFAULT_MAX_HITS))
}

fn user(login: &str, name: Option<&str>, email: &str, avatar_url: &str) -> MentionableUser {
    MentionableUser {
        login: login.to_string(),
        name: name.map(str::to_string),
        email: Some(email.to_string()),
        avatar_url: Some(avatar_url.to_string()),
    }
}

fn mona() -> MentionableUser {
    user(
        "octocat",
        Some("Mona Lisa"),
        "octocat@example.com",
        "https://avatars.example.com/octocat",
    )
}

/// A user that hasn't configured a real name (name === null).
fn nameless_user() -> MentionableUser {
    user(
        "hubot",
        None,
        "hubot@example.com",
        "https://avatars.example.com/hubot",
    )
}

fn users() -> Vec<MentionableUser> {
    vec![mona(), nameless_user()]
}

fn logins(hits: &[MentionableUser]) -> Vec<&str> {
    hits.iter().map(|u| u.login.as_str()).collect()
}

// GHD: unit/stores/github-user-store-test.ts › matchMentionableUsers › does not match users without a name when searching for "null"
#[test]
fn does_not_match_users_without_a_name_when_searching_for_null() {
    // Before the fix, a `null` name was interpolated as the literal string
    // "null", causing nameless users to match the query "null".
    assert_eq!(
        match_mentionable_users(&users(), "null", None),
        Vec::<MentionableUser>::new()
    );
}

// GHD: unit/stores/github-user-store-test.ts › matchMentionableUsers › does not drag in a nameless user on a leading "n"
#[test]
fn does_not_drag_in_a_nameless_user_on_a_leading_n() {
    let hits = match_mentionable_users(&users(), "n", None);
    let logins = logins(&hits);

    // "octocat Mona Lisa" legitimately contains an "n" (Mona); the nameless
    // "hubot" must not be matched via the literal "null".
    assert!(!logins.contains(&"hubot"));
}

// GHD: unit/stores/github-user-store-test.ts › matchMentionableUsers › matches a user by their login even when they have no name
#[test]
fn matches_a_user_by_their_login_even_when_they_have_no_name() {
    let hits = match_mentionable_users(&users(), "hubot", None);

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].login, "hubot");
}

// GHD: unit/stores/github-user-store-test.ts › matchMentionableUsers › matches a user by their real name
#[test]
fn matches_a_user_by_their_real_name() {
    let hits = match_mentionable_users(&users(), "mona", None);

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].login, "octocat");
}

// GHD: unit/stores/github-user-store-test.ts › matchMentionableUsers › matches a user by their login
#[test]
fn matches_a_user_by_their_login() {
    let hits = match_mentionable_users(&users(), "octo", None);

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].login, "octocat");
}

// GHD: unit/stores/github-user-store-test.ts › matchMentionableUsers › ranks earlier matches ahead of later ones
#[test]
fn ranks_earlier_matches_ahead_of_later_ones() {
    let scarecrow = user(
        "scarecrow",
        None,
        "scarecrow@example.com",
        "https://avatars.example.com/scarecrow",
    );
    let oscar = user(
        "oscar",
        None,
        "oscar@example.com",
        "https://avatars.example.com/oscar",
    );

    // "scar" starts at index 0 of "scarecrow" but at index 1 of "oscar", so
    // scarecrow should rank first.
    let hits = match_mentionable_users(&[oscar, scarecrow], "scar", None);

    assert_eq!(logins(&hits), vec!["scarecrow", "oscar"]);
}

// GHD: unit/stores/github-user-store-test.ts › matchMentionableUsers › honors the maxHits limit
#[test]
fn honors_the_max_hits_limit() {
    let many: Vec<MentionableUser> = (0..5)
        .map(|i| {
            user(
                &format!("matcher{i}"),
                None,
                &format!("matcher{i}@example.com"),
                &format!("https://avatars.example.com/matcher{i}"),
            )
        })
        .collect();

    assert_eq!(match_mentionable_users(&many, "matcher", Some(3)).len(), 3);
}

// GHD: unit/stores/github-user-store-test.ts › matchMentionableUsers › ranks a login match ahead of a name-only match
#[test]
fn ranks_a_login_match_ahead_of_a_name_only_match() {
    let login_match = user(
        "scarf",
        None,
        "scarf@example.com",
        "https://avatars.example.com/scarf",
    );
    let name_match = user(
        "zzz",
        Some("Scar Face"),
        "zzz@example.com",
        "https://avatars.example.com/zzz",
    );

    // "scar" matches at index 0 of login "scarf" but at index 4 of the haystack
    // "zzz scar face" (i.e. the "login " prefix is still part of the search
    // string), so the login match must rank first.
    let hits = match_mentionable_users(&[name_match, login_match], "scar", None);

    assert_eq!(logins(&hits), vec!["scarf", "zzz"]);
}

// GHD: unit/stores/github-user-store-test.ts › matchMentionableUsers › treats an empty-string name the same as a null name
#[test]
fn treats_an_empty_string_name_the_same_as_a_null_name() {
    let with_empty_name = user(
        "foo",
        Some(""),
        "foo@example.com",
        "https://avatars.example.com/foo",
    );
    let with_null_name = MentionableUser {
        name: None,
        ..with_empty_name.clone()
    };

    // Both forms match by login and neither one is dragged in by "null".
    assert_eq!(
        match_mentionable_users(std::slice::from_ref(&with_empty_name), "foo", None).len(),
        1
    );
    assert_eq!(
        match_mentionable_users(&[with_null_name], "foo", None).len(),
        1
    );
    assert_eq!(
        match_mentionable_users(&[with_empty_name], "null", None).len(),
        0
    );
}

// GHD: unit/stores/github-user-store-test.ts › matchMentionableUsers › breaks ties alphabetically by login
#[test]
fn breaks_ties_alphabetically_by_login() {
    let team_zoo = user(
        "team-zoo",
        None,
        "team-zoo@example.com",
        "https://avatars.example.com/team-zoo",
    );
    let team_ant = user(
        "team-ant",
        None,
        "team-ant@example.com",
        "https://avatars.example.com/team-ant",
    );

    // Both match "team" at index 0, so the secondary alphabetical-by-login
    // sort decides the order.
    let hits = match_mentionable_users(&[team_zoo, team_ant], "team", None);

    assert_eq!(logins(&hits), vec!["team-ant", "team-zoo"]);
}

// GHD: unit/stores/github-user-store-test.ts › matchMentionableUsers › matches case-insensitively for uppercase queries
#[test]
fn matches_case_insensitively_for_uppercase_queries() {
    let hits = match_mentionable_users(&users(), "MONA", None);

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].login, "octocat");
}
