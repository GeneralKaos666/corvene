//! The Issues and Releases calls (flags `345-issues`, `346-releases`)
//! against the stub HTTP server: what they send and how they decode
//! GitHub's answers, including the fields GitHub Enterprise Server leaves
//! out.

use corvene_github::{Client, Endpoint, NewIssue, NewRelease};
use corvene_test_support::{StubResponse, serve, serve_with};
use serde_json::json;

fn client(url: &str) -> Client {
    Client::new(
        Endpoint::from_api_base(&format!("{url}/api/v3")),
        "token".to_string(),
    )
}

#[test]
fn issues_decode_with_and_without_the_newer_fields() {
    let body = json!([
        {
            "number": 42, "title": "Crash", "state": "open", "updated_at": "2024-03-01T10:00:00Z",
            "id": 1042, "node_id": "I_42", "body": "Steps", "html_url": "https://ghe/o/r/issues/42",
            "created_at": "2024-03-01T09:00:00Z", "user": {"login": "octocat"},
            "labels": [{"name": "bug", "color": "d73a4a", "description": null}],
            "assignees": [{"login": "mona"}], "comments": 2, "state_reason": null
        },
        // an older GitHub Enterprise Server: no `state_reason`, no `node_id`
        {"number": 7, "title": "Old", "state": "closed", "updated_at": "2024-01-01T00:00:00Z"},
        // a pull request is an issue too; it is dropped
        {"number": 8, "title": "PR", "state": "open", "updated_at": "2024-01-01T00:00:00Z", "pull_request": {}}
    ]);
    let server = serve_with(move |_| StubResponse::new(200, body.to_string()));
    let issues = client(server.url())
        .closed_issues("o", "r", 2)
        .expect("issues");
    assert_eq!(issues.len(), 2);
    let crash = &issues[0];
    assert_eq!(crash.node_id.as_deref(), Some("I_42"));
    assert_eq!(crash.labels[0].name, "bug");
    assert_eq!(crash.assignees[0].login, "mona");
    assert_eq!(crash.comments, 2);
    let old = &issues[1];
    assert_eq!(old.state_reason, None);
    assert!(old.labels.is_empty());
    assert_eq!(old.body, None);
    let request = &server.requests()[0];
    assert!(
        request
            .target
            .starts_with("/api/v3/repos/o/r/issues?state=closed"),
        "{}",
        request.target
    );
}

#[test]
fn create_issue_posts_the_form() {
    let server = serve_with(|request| {
        assert!(
            request.head.starts_with("POST /api/v3/repos/o/r/issues "),
            "{}",
            request.head
        );
        let sent: serde_json::Value = serde_json::from_slice(&request.body).expect("json body");
        assert_eq!(sent["title"], "Crash on launch");
        assert_eq!(sent["labels"], json!(["bug"]));
        assert_eq!(sent["assignees"], json!(["mona"]));
        StubResponse::new(
            201,
            json!({"number": 101, "title": "Crash on launch", "state": "open", "updated_at": "2024-05-01T10:00:00Z",
                   "html_url": "https://ghe/o/r/issues/101"})
            .to_string(),
        )
    });
    let issue = client(server.url())
        .create_issue(
            "o",
            "r",
            &NewIssue {
                title: "Crash on launch".to_string(),
                body: String::new(),
                labels: vec!["bug".to_string()],
                assignees: vec!["mona".to_string()],
            },
        )
        .expect("created");
    assert_eq!(issue.number, 101);
    assert_eq!(
        issue.html_url.as_deref(),
        Some("https://ghe/o/r/issues/101")
    );
}

#[test]
fn releases_decode_assets_and_create_sends_the_draft() {
    let body = json!([{
        "id": 1, "tag_name": "v0.1.0", "name": "First", "body": "notes", "html_url": "https://ghe/o/r/releases/tag/v0.1.0",
        "draft": false, "prerelease": true, "author": {"login": "octocat"}, "created_at": "2024-02-15T10:00:00Z",
        "published_at": "2024-02-15T10:00:00Z", "target_commitish": "main", "tarball_url": "https://ghe/o/r/tar",
        "assets": [{"name": "app.dmg", "size": 10, "browser_download_url": "https://ghe/o/r/app.dmg", "download_count": 3}]
    }]);
    let url = serve(StubResponse::new(200, body.to_string()));
    let releases = client(&url).releases("o", "r", 1).expect("releases");
    assert_eq!(releases.len(), 1);
    assert!(releases[0].prerelease);
    assert_eq!(releases[0].assets[0].name, "app.dmg");
    assert_eq!(
        releases[0].author.as_ref().map(|a| a.login.as_str()),
        Some("octocat")
    );

    let server = serve_with(|request| {
        let sent: serde_json::Value = serde_json::from_slice(&request.body).expect("json body");
        assert_eq!(sent["tag_name"], "v0.2.0");
        assert_eq!(sent["target_commitish"], "abc123");
        assert_eq!(sent["draft"], true);
        assert_eq!(sent.get("name"), None, "an empty title is not sent");
        StubResponse::new(
            201,
            json!({"id": 9, "tag_name": "v0.2.0", "html_url": "https://ghe/o/r/releases/tag/v0.2.0", "draft": true})
                .to_string(),
        )
    });
    let release = client(server.url())
        .create_release(
            "o",
            "r",
            &NewRelease {
                tag_name: "v0.2.0".to_string(),
                target_commitish: Some("abc123".to_string()),
                name: None,
                body: "notes".to_string(),
                draft: true,
                prerelease: false,
            },
        )
        .expect("created");
    assert!(release.draft);
    assert_eq!(release.id, 9);
}

#[test]
fn generate_notes_falls_back_where_the_host_cannot() {
    let server = serve_with(|request| {
        let sent: serde_json::Value = serde_json::from_slice(&request.body).expect("json body");
        assert_eq!(sent["tag_name"], "v0.2.0");
        assert_eq!(sent["previous_tag_name"], "v0.1.0");
        StubResponse::new(
            200,
            json!({"name": "v0.2.0", "body": "## What's Changed"}).to_string(),
        )
    });
    let notes = client(server.url())
        .generate_release_notes("o", "r", "v0.2.0", None, Some("v0.1.0"))
        .expect("generated");
    assert_eq!(notes.map(|n| n.body), Some("## What's Changed".to_string()));

    // GitHub Enterprise Server before 3.5 has no such endpoint
    let url = serve(StubResponse::new(
        404,
        json!({"message": "Not Found"}).to_string(),
    ));
    assert_eq!(
        client(&url)
            .generate_release_notes("o", "r", "v0.2.0", None, None)
            .expect("no error"),
        None
    );
    // an unreadable tag
    let url = serve(StubResponse::new(
        422,
        json!({"message": "Validation Failed"}).to_string(),
    ));
    assert_eq!(
        client(&url)
            .generate_release_notes("o", "r", "v0.2.0", None, None)
            .expect("no error"),
        None
    );
}

#[test]
fn linked_branch_reads_the_graphql_answer() {
    let url = serve(StubResponse::new(
        200,
        json!({"data": {"createLinkedBranch": {"linkedBranch": {"id": "LB_1"}}}}).to_string(),
    ));
    assert!(
        client(&url)
            .create_linked_branch("I_1", "R_1", "abc", "42-crash")
            .expect("linked")
    );
    let url = serve(StubResponse::new(
        200,
        json!({"data": null, "errors": [{"message": "Resource not accessible by integration"}]})
            .to_string(),
    ));
    assert!(
        client(&url)
            .create_linked_branch("I_1", "R_1", "abc", "42-crash")
            .is_err()
    );
}
