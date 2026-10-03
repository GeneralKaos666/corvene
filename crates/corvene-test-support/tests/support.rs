//! Checks of the test support itself: the environment isolation and that
//! each helper builds what its GitHub Desktop original builds.

use std::path::Path;

use corvene_models::FileStatusKind;
use corvene_test_support::*;

fn lines(text: &str) -> Vec<&str> {
    text.lines().collect()
}

fn log_subjects(repo: &TestRepo, rev: &str) -> Vec<String> {
    exec_ok(["log", "--format=%s", rev], repo.path())
        .stdout
        .lines()
        .map(str::to_string)
        .collect()
}

fn current_branch(repo: &TestRepo) -> String {
    exec_ok(["symbolic-ref", "--short", "HEAD"], repo.path())
        .stdout
        .trim()
        .to_string()
}

fn canonical(path: impl AsRef<Path>) -> std::path::PathBuf {
    std::fs::canonicalize(path).expect("canonicalize")
}

fn read(path: impl AsRef<Path>) -> String {
    std::fs::read_to_string(path).expect("read file")
}

#[test]
fn environment_was_isolated_before_main() {
    assert!(isolated_before_main());
}

#[test]
fn environment_points_home_at_an_empty_temp_dir() {
    init();
    let home = home_dir();
    assert_eq!(std::env::var_os("HOME").as_deref(), Some(home.as_os_str()));
    assert_eq!(
        std::env::var_os("USERPROFILE").as_deref(),
        Some(home.as_os_str())
    );
    // the XDG global config follows `HOME`, as in GitHub Desktop's CI
    assert_eq!(std::env::var_os("XDG_CONFIG_HOME"), None);
    assert!(
        home.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("corvene-test-home-")
    );
    assert_eq!(
        canonical(home.parent().unwrap()),
        canonical(std::env::temp_dir())
    );
    assert_eq!(std::env::var("GIT_CONFIG_NOSYSTEM").as_deref(), Ok("1"));
    assert_eq!(std::env::var("TERM").as_deref(), Ok("dumb"));
    for key in ["EDITOR", "VISUAL"] {
        assert_eq!(std::env::var_os(key), None, "{key}");
    }
    for key in ["GIT_AUTHOR_NAME", "GIT_COMMITTER_NAME"] {
        assert_eq!(std::env::var(key).as_deref(), Ok("Joe Bloggs"));
    }
    for key in ["GIT_AUTHOR_EMAIL", "GIT_COMMITTER_EMAIL"] {
        assert_eq!(
            std::env::var(key).as_deref(),
            Ok("joe.bloggs@somewhere.com")
        );
    }
    let allowed = [
        "GIT_CONFIG_NOSYSTEM",
        "GIT_AUTHOR_NAME",
        "GIT_AUTHOR_EMAIL",
        "GIT_COMMITTER_NAME",
        "GIT_COMMITTER_EMAIL",
    ];
    let inherited: Vec<_> = std::env::vars_os()
        .map(|(k, _)| k.to_string_lossy().into_owned())
        .filter(|k| k.starts_with("GIT_") && !allowed.contains(&k.as_str()))
        .collect();
    assert!(inherited.is_empty(), "{inherited:?}");
}

#[test]
fn user_configuration_is_invisible_in_a_helper_repository() {
    let repo = setup_empty_repository();
    // the developer's ~/.gitconfig sets commit.gpgsign; tests must not see it
    let gpgsign = exec(["config", "--get", "commit.gpgsign"], repo.path());
    assert_eq!(gpgsign.exit_code, 1, "{gpgsign:?}");
    assert_eq!(gpgsign.stdout, "");
    // nothing but the repository's own config is read
    let origins = exec_ok(["config", "--list", "--show-origin"], repo.path()).stdout;
    for line in origins.lines() {
        assert!(line.starts_with("file:.git/config\t"), "{line}");
    }
    // `$XDG_CONFIG_HOME/git/config` and `$HOME/.gitconfig`
    let globals = exec_ok(["var", "GIT_CONFIG_GLOBAL"], repo.path()).stdout;
    assert_eq!(globals.lines().count(), 2, "{globals}");
    for global in globals.lines() {
        let existing = Path::new(global).ancestors().find(|p| p.exists()).unwrap();
        assert!(
            canonical(existing).starts_with(canonical(home_dir())),
            "global config at {global}"
        );
    }
    // gix (Corvene's reads) does not see a user identity either
    let info = corvene_git::open_repository(repo.path()).unwrap();
    assert_eq!(info.identity.name, None);
    assert_eq!(info.identity.email, None);
}

#[test]
fn a_home_passed_to_one_command_moves_all_global_config() {
    // GitHub Desktop's config tests pass `env: { HOME }` to one command
    let dir = setup_empty_directory();
    let other_home = create_temp_directory();
    let globals = exec_with(
        ["var", "GIT_CONFIG_GLOBAL"],
        dir.path(),
        ExecOptions {
            env: vec![("HOME".into(), other_home.path().into())],
            ..Default::default()
        },
    );
    assert_eq!(globals.exit_code, 0, "{globals:?}");
    // `$HOME/.config/git/config` (no `XDG_CONFIG_HOME`) and `$HOME/.gitconfig`
    let globals = lines(&globals.stdout);
    assert_eq!(globals.len(), 2, "{globals:?}");
    for global in globals {
        assert!(Path::new(global).starts_with(other_home.path()), "{global}");
    }
}

#[test]
fn commits_use_the_test_identity() {
    let repo = setup_empty_repository();
    make_commit(&repo, &Tree::new([TreeEntry::new("a", "a")]));
    let who = exec_ok(
        ["log", "-1", "--format=%an <%ae>|%cn <%ce>|%s"],
        repo.path(),
    );
    assert_eq!(
        who.stdout.trim(),
        "Joe Bloggs <joe.bloggs@somewhere.com>|Joe Bloggs <joe.bloggs@somewhere.com>|commit"
    );
    // not signed, whatever the developer's configuration says
    let raw = exec_ok(["cat-file", "commit", "HEAD"], repo.path()).stdout;
    assert!(!raw.contains("gpgsig"), "{raw}");
}

#[test]
fn commands_that_want_an_editor_fail_at_once() {
    let repo = setup_empty_repository();
    let result = exec(["commit", "--allow-empty"], repo.path());
    assert_ne!(result.exit_code, 0, "{result:?}");
    assert!(
        result.stderr.contains("Terminal is dumb, but EDITOR unset"),
        "{result:?}"
    );
}

/// The global configuration file reads as no configuration.
fn empty_or_missing(path: &Path) -> bool {
    std::fs::read(path).map_or(true, |bytes| bytes.is_empty())
}

#[test]
fn global_config_lock_starts_and_ends_empty() {
    let path;
    {
        let guard = lock_global_config();
        path = guard.path();
        assert!(empty_or_missing(&path));
        let dir = create_temp_directory();
        exec_ok(
            ["config", "--global", "init.defaultBranch", "trunk"],
            dir.path(),
        );
        assert!(!empty_or_missing(&path));
        let value = exec(
            ["config", "--global", "--get", "init.defaultBranch"],
            dir.path(),
        );
        assert_eq!(value.stdout.trim(), "trunk");
    }
    assert!(empty_or_missing(&path));
}

#[test]
fn emptying_the_global_config_never_removes_it() {
    // what used to break unrelated clones: git checks that the global
    // configuration exists, then opens it; removing it in between kills git
    let dir = create_temp_directory();
    {
        let _guard = lock_global_config();
        exec_ok(["config", "--global", "protocol.version", "0"], dir.path());
    }
    let guard = lock_global_config();
    assert!(guard.path().exists(), "the emptied file stays");
    assert!(empty_or_missing(&guard.path()));
}

#[test]
fn proxies_are_off_for_the_stub_servers() {
    for name in [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
    ] {
        assert!(std::env::var_os(name).is_none(), "{name} is set");
    }
    assert_eq!(
        std::env::var("NO_PROXY").as_deref(),
        Ok("127.0.0.1,localhost,::1")
    );
}

#[test]
fn stub_servers_answer_route_and_record() {
    use std::io::{Read, Write};

    fn get(url: &str, path: &str) -> String {
        let address = url.trim_start_matches("http://");
        let mut stream = std::net::TcpStream::connect(address).expect("connect");
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nHost: {address}\r\nX-Test: yes\r\n\r\n"
        )
        .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response
    }

    let url = serve(StubResponse::new(404, "{}").with_header("Content-Type", "application/json"));
    let response = get(&url, "/x");
    assert!(
        response.starts_with("HTTP/1.1 404 Not Found\r\n"),
        "{response}"
    );
    assert!(
        response.contains("Content-Type: application/json\r\n"),
        "{response}"
    );
    assert!(response.ends_with("\r\n\r\n{}"), "{response}");

    let server = serve_with(|request| match request.path() {
        "/user" => StubResponse::new(200, "me"),
        _ => StubResponse::new(500, "").with_status_text("Nope"),
    });
    assert!(get(server.url(), "/user?a=1").ends_with("\r\n\r\nme"));
    assert!(get(server.url(), "/other").starts_with("HTTP/1.1 500 Nope\r\n"));
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].target, "/user?a=1");
    assert_eq!(requests[0].header("x-test"), Some("yes"));
    assert!(server.request_heads()[1].starts_with("GET /other HTTP/1.1\r\n"));

    let unreachable = unreachable_endpoint();
    let address = unreachable.trim_start_matches("http://");
    assert!(std::net::TcpStream::connect(address).is_err());
}

#[test]
fn dates_parse_and_format_like_javascript() {
    let date = date_parse("2026-03-26T12:00:00.000Z");
    assert_eq!(
        date,
        std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_774_526_400)
    );
    assert_eq!(date_parse("2026-03-26T12:00:00Z"), date);
    assert_eq!(to_iso_string(date), "2026-03-26T12:00:00.000Z");
    let date = date_parse("2024-02-29T23:59:58.25Z");
    assert_eq!(to_iso_string(date), "2024-02-29T23:59:58.250Z");
    assert_eq!(
        to_iso_string(std::time::UNIX_EPOCH),
        "1970-01-01T00:00:00.000Z"
    );
}

#[test]
fn base64_pads_like_buffer() {
    assert_eq!(base64(b""), "");
    assert_eq!(base64(b"f"), "Zg==");
    assert_eq!(base64(b"fo"), "Zm8=");
    assert_eq!(base64(b"foo"), "Zm9v");
    assert_eq!(base64(&[0xff, 0xfe, 0xfd, 0xfc]), "//79/A==");
}

#[test]
fn get_branches_filters_by_prefix_and_ignores_non_repositories() {
    let repo = setup_two_commit_repo();
    exec_ok(["branch", "feature/one"], repo.path());
    exec_ok(["branch", "feature-two"], repo.path());
    let names = |prefixes: &[&str]| -> Vec<String> {
        let mut names: Vec<String> = get_branches(repo.path(), prefixes)
            .into_iter()
            .map(|b| b.full_name)
            .collect();
        names.sort();
        names
    };
    assert_eq!(names(&["refs/heads/feature"]), ["refs/heads/feature/one"]);
    assert_eq!(
        names(&["refs/heads/feature-two"]),
        ["refs/heads/feature-two"]
    );
    assert_eq!(names(&[]).len(), 3);
    let not_a_repo = setup_empty_directory();
    assert!(get_branches(not_a_repo.path(), &[]).is_empty());
}

#[test]
fn git_hub_repo_fixture_builds_like_github_desktop() {
    let parent = git_hub_repo_fixture(GitHubRepoFixtureOptions {
        owner: "desktop",
        name: "desktop",
        ..Default::default()
    });
    assert_eq!(parent.endpoint, DOT_COM_API_ENDPOINT);
    assert_eq!(parent.html_url, "https://github.com/desktop/desktop");
    assert_eq!(parent.clone_url, "https://github.com/desktop/desktop.git");
    assert!(!parent.fork && !parent.private);
    let fork = git_hub_repo_fixture(GitHubRepoFixtureOptions {
        owner: "me",
        name: "desktop",
        parent: Some(parent.clone()),
        is_private: Some(true),
        endpoint: Some("https://ghe.io"),
    });
    assert_eq!(fork.endpoint, "https://ghe.io");
    assert_eq!(fork.html_url, "https://ghe.io/me/desktop");
    assert!(fork.fork && fork.private);
    assert_eq!(fork.parent.as_deref(), Some(&parent));
}

#[test]
fn exec_returns_non_zero_exit_codes() {
    let repo = setup_empty_directory();
    let result = exec(["rev-parse", "--verify", "HEAD"], repo.path());
    assert_eq!(result.exit_code, 128);
    assert!(result.stderr.contains("not a git repository"), "{result:?}");
}

#[test]
fn exec_with_passes_stdin_and_env() {
    let repo = setup_empty_repository();
    let result = exec_with(
        ["hash-object", "--stdin"],
        repo.path(),
        ExecOptions {
            stdin: Some(b"hello\n".to_vec()),
            ..Default::default()
        },
    );
    assert_eq!(result.exit_code, 0);
    assert_eq!(
        result.stdout.trim(),
        "ce013625030ba8dba906f756967f9e9ca394464a"
    );

    let result = exec_with(
        ["var", "GIT_AUTHOR_IDENT"],
        repo.path(),
        ExecOptions {
            env: vec![("GIT_AUTHOR_NAME".into(), "Someone Else".into())],
            ..Default::default()
        },
    );
    assert!(result.stdout.starts_with("Someone Else <"), "{result:?}");
}

#[test]
#[should_panic(expected = "failed")]
fn exec_ok_panics_on_failure() {
    let dir = setup_empty_directory();
    exec_ok(["rev-parse", "HEAD"], dir.path());
}

#[test]
fn temp_directories_are_prefixed_and_removed() {
    let dir = create_temp_directory();
    let path = dir.path().to_path_buf();
    assert!(
        path.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("desktop-test-")
    );
    assert!(path.is_dir());
    drop(dir);
    assert!(!path.exists());
}

#[test]
fn empty_repository_has_the_ghd_skeleton() {
    let repo = setup_empty_repository();
    let git = repo.git_dir();
    for dir in ["objects", "refs", "refs/tags", "refs/heads", "info"] {
        assert!(git.join(dir).is_dir(), "{dir}");
    }
    assert_eq!(read(git.join("HEAD")), "ref: refs/heads/master\n");
    let ignore_case = if cfg!(target_os = "linux") {
        "true"
    } else {
        "false"
    };
    assert_eq!(
        read(git.join("config")),
        format!(
            "[core]\nrepositoryformatversion = 0\nfilemode = true\nbare = false\nlogallrefupdates = true\nignorecase = {ignore_case}\nprecomposeunicode = true\n"
        )
    );
    assert_eq!(
        read(git.join("description")),
        "Unnamed repository; edit this file 'description' to name the repository.\n"
    );
    assert_eq!(read(git.join("description")), DEFAULT_GIT_DESCRIPTION);
    let status = get_status_or_throw(&repo);
    assert!(status.files.is_empty());
    assert_eq!(status.branch.as_deref(), Some("master"));

    let main = setup_empty_repository_default_main();
    assert_eq!(read(main.git_dir().join("HEAD")), "ref: refs/heads/main\n");
    let other = setup_empty_repository_with_default_branch("trunk");
    assert_eq!(current_branch(&other), "trunk");
}

#[test]
fn empty_directory_is_not_a_repository() {
    let dir = setup_empty_directory();
    assert!(dir.path().is_dir());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    assert!(corvene_git::get_status(git(), dir.path(), None).is_err());
}

#[test]
fn fixture_repositories_get_their_git_dirs_renamed() {
    let repo = setup_fixture_repository("test-repo");
    assert!(repo.git_dir().is_dir());
    assert!(!repo.join("_git").exists());
    let tip = get_tip_or_error(&repo);
    assert_eq!(tip.sha.len(), 40);
    assert!(get_status_or_throw(&repo).files.is_empty());

    // nested `_git`, here a gitlink file, is renamed too
    let repo = setup_fixture_repository("submodule-basic-setup");
    assert!(repo.git_dir().is_dir());
    let gitlink = repo.join("foo/submodule/.git");
    assert!(gitlink.is_file());
    assert!(!repo.join("foo/submodule/_git").exists());
    assert!(read(gitlink).starts_with("gitdir: "));
    let submodule = repo.subdirectory("foo/submodule");
    assert_eq!(submodule.path(), repo.join("foo/submodule"));
    assert_eq!(
        canonical(
            exec_ok(["rev-parse", "--show-toplevel"], submodule.path())
                .stdout
                .trim()
        ),
        canonical(submodule.path())
    );

    // the fixtures themselves are never touched
    assert!(get_fixture_path(["test-repo", "_git"]).is_dir());
    assert!(!get_fixture_path(["test-repo", ".git"]).exists());
    assert_eq!(fixtures_dir(), get_fixture_path::<_, &str>([]));
}

#[test]
fn every_fixture_repository_opens() {
    for entry in std::fs::read_dir(fixtures_dir()).unwrap() {
        let entry = entry.unwrap();
        if !entry.file_type().unwrap().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let repo = setup_fixture_repository(&name);
        let result = exec(["rev-parse", "--git-dir"], repo.path());
        assert_eq!(result.exit_code, 0, "{name}: {result:?}");
        assert_eq!(result.stdout.trim(), ".git", "{name}");
    }
}

#[test]
fn make_commit_writes_removes_and_names_commits() {
    let repo = setup_empty_repository();
    make_commit(
        &repo,
        &Tree::with_message(
            "",
            [
                TreeEntry::new("a", "1"),
                TreeEntry::new("b", b"\x00\x01".to_vec()),
            ],
        ),
    );
    make_commit(
        &repo,
        &Tree::with_message("second", [TreeEntry::removed("a")]),
    );
    assert_eq!(log_subjects(&repo, "HEAD"), ["second", "commit"]);
    assert!(!repo.join("a").exists());
    assert_eq!(std::fs::read(repo.join("b")).unwrap(), b"\x00\x01");
    assert_eq!(lines(&exec_ok(["ls-files"], repo.path()).stdout), ["b"]);
}

#[test]
fn branches_are_created_and_switched_to() {
    let repo = setup_two_commit_repo();
    assert_eq!(log_subjects(&repo, "HEAD"), ["commit", "commit"]);
    assert_eq!(read(repo.join("good-file")), "is great");
    assert_eq!(read(repo.join("great-file")), "is good");

    create_branch(&repo, "topic", "HEAD~1");
    assert_eq!(current_branch(&repo), "master");
    assert_eq!(
        get_branch_or_error(&repo, "topic").tip.as_deref(),
        Some(get_ref_or_error(&repo, "HEAD~1").sha.as_str())
    );
    switch_to(&repo, "topic");
    assert_eq!(current_branch(&repo), "topic");
    switch_to(&repo, "new-one");
    assert_eq!(current_branch(&repo), "new-one");
    assert_eq!(
        get_tip_or_error(&repo).sha,
        get_ref_or_error(&repo, "topic").sha
    );
}

#[test]
#[should_panic(expected = "Branch master already exists")]
fn create_branch_refuses_an_existing_branch() {
    let repo = setup_two_commit_repo();
    create_branch(&repo, "master", "HEAD");
}

#[test]
#[should_panic(expected = "Unable to find commit for HEAD")]
fn tip_of_an_unborn_repository_is_an_error() {
    get_tip_or_error(&setup_empty_repository());
}

#[test]
#[should_panic(expected = "Unable to find branch matching refs/heads/nope")]
fn missing_branch_is_an_error() {
    get_branch_or_error(&setup_two_commit_repo(), "nope");
}

#[test]
fn conflicted_repo_conflicts_in_foo() {
    let repo = setup_conflicted_repo();
    assert_eq!(current_branch(&repo), "other-branch");
    let status = get_status_or_throw(&repo);
    assert!(status.merge_head_found);
    let conflicted: Vec<_> = status
        .files
        .iter()
        .filter(|f| f.status.kind == FileStatusKind::Conflicted)
        .map(|f| f.path.as_str())
        .collect();
    assert_eq!(conflicted, ["foo"]);
}

#[test]
fn conflicted_repo_with_unrelated_change_keeps_perlin_modified() {
    let repo = setup_conflicted_repo_with_unrelated_committed_change();
    assert_eq!(read(repo.join("perlin")), "noise");
    let status = get_status_or_throw(&repo);
    let kinds: Vec<_> = status
        .files
        .iter()
        .map(|f| (f.path.as_str(), f.status.kind))
        .collect();
    assert_eq!(
        kinds,
        [
            ("foo", FileStatusKind::Conflicted),
            ("perlin", FileStatusKind::Modified)
        ]
    );
}

#[test]
fn conflicted_repo_with_multiple_files() {
    let repo = setup_conflicted_repo_with_multiple_files();
    assert_eq!(current_branch(&repo), "other-branch");
    let status = get_status_or_throw(&repo);
    let kinds: Vec<_> = status
        .files
        .iter()
        .map(|f| (f.path.as_str(), f.status.kind))
        .collect();
    assert_eq!(
        kinds,
        [
            ("bar", FileStatusKind::Conflicted),
            ("baz", FileStatusKind::Conflicted),
            ("cat", FileStatusKind::Conflicted),
            ("dog", FileStatusKind::Untracked),
            ("foo", FileStatusKind::Conflicted),
        ]
    );
}

#[test]
fn clones_and_forks_point_at_their_source() {
    let upstream = setup_two_commit_repo();
    for clone in [
        clone_repository(&upstream),
        clone_local_repository(&upstream),
        setup_local_fork_of_repository(&upstream),
    ] {
        let url = exec_ok(["remote", "get-url", "origin"], clone.path()).stdout;
        assert_eq!(canonical(url.trim()), canonical(upstream.path()));
        assert_eq!(
            get_tip_or_error(&clone).sha,
            get_tip_or_error(&upstream).sha
        );
    }
}

#[test]
fn clones_keep_their_source_alive() {
    let clone = clone_repository(&setup_two_commit_repo());
    let fetch = exec(["fetch", "origin"], clone.path());
    assert_eq!(fetch.exit_code, 0, "{fetch:?}");
}

#[test]
fn uninitialized_submodule_lives_on_a_branch() {
    let repo = setup_repository_with_uninitialized_submodule();
    assert_eq!(current_branch(&repo), "master");
    assert!(!repo.join("test-submodule").exists());
    assert!(!repo.join(".git/modules/test-submodule").exists());
    assert_eq!(
        log_subjects(&repo, "branch-with-submodule"),
        ["Add submodule", "commit", "commit"]
    );
    let tree = exec_ok(["ls-tree", "branch-with-submodule"], repo.path()).stdout;
    assert!(tree.contains("160000 commit"), "{tree}");
    // its source is still there to initialize it from
    exec_ok(["checkout", "branch-with-submodule"], repo.path());
    let update = exec(
        [
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "update",
            "--init",
        ],
        repo.path(),
    );
    assert_eq!(update.exit_code, 0, "{update:?}");
    assert!(repo.join("test-submodule/good-file").is_file());
}

#[test]
fn local_config_is_written() {
    let repo = setup_empty_repository();
    setup_local_config(&repo, [("user.name", "Local"), ("core.autocrlf", "input")]);
    let value = exec_ok(["config", "--local", "--get", "user.name"], repo.path());
    assert_eq!(value.stdout.trim(), "Local");
    let value = exec_ok(["config", "--local", "--get", "core.autocrlf"], repo.path());
    assert_eq!(value.stdout.trim(), "input");
}

#[test]
fn generated_strings_are_hex() {
    for length in [0, 1, 7, DEFAULT_STRING_LENGTH] {
        let s = generate_string(length);
        assert_eq!(s.len(), length);
        assert!(
            s.bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        );
    }
    assert_ne!(generate_string(32), generate_string(32));
}

#[test]
fn rebase_builder() {
    let repo = repository_builder_rebase::create_repository("base-branch", "feature-branch");
    assert_eq!(current_branch(&repo), "master");
    assert_eq!(log_subjects(&repo, "master"), ["Second!", "First!"]);
    assert_eq!(
        log_subjects(&repo, "base-branch"),
        ["Base Branch!", "Second!", "First!"]
    );
    assert_eq!(
        log_subjects(&repo, "feature-branch"),
        ["Feature Branch!", "Second!", "First!"]
    );
    let thing = exec_ok(["show", "feature-branch:THING.md"], repo.path()).stdout;
    assert_eq!(
        thing,
        "# HELLO WORLD! \nTHINGS GO HERE\nFEATURE BRANCH UNDERWAY\n"
    );
    assert_eq!(read(repo.join("THIRD.md")), "nothing goes here");
}

#[test]
fn long_rebase_builder() {
    let repo = repository_builder_long_rebase::create_repository("base-branch", "feature-branch");
    assert_eq!(current_branch(&repo), "master");
    assert_eq!(log_subjects(&repo, "master..base-branch").len(), 3);
    let feature = log_subjects(&repo, "master..feature-branch");
    assert_eq!(feature.len(), 10);
    assert_eq!(feature[0], "Feature Branch Tenth Commit!");
    assert_eq!(feature[3], "Feature Branch Third Commit!");
    assert_eq!(feature[7], "Feature Branch Third Commit!");
    assert_eq!(feature[9], "Feature Branch First Commit!");
}

#[test]
fn cherry_pick_builder() {
    let repo = repository_builder_cherry_pick::create_repository("feature-branch", "target-branch");
    assert_eq!(current_branch(&repo), "target-branch");
    assert_eq!(log_subjects(&repo, "main"), ["First!"]);
    assert_eq!(log_subjects(&repo, "target-branch"), ["First!"]);
    assert_eq!(
        log_subjects(&repo, "feature-branch"),
        ["Cherry-picked Feature!", "First!"]
    );
}

#[test]
fn pull_builder() {
    let repo = repository_builder_pull::create_repository("some-branch");
    assert_eq!(current_branch(&repo), "some-branch");
    assert_eq!(
        log_subjects(&repo, "some-branch"),
        ["Updated README", "Added a new file", "Second!", "First!"]
    );
    assert_eq!(log_subjects(&repo, "master"), ["Second!", "First!"]);
    assert_eq!(read(repo.join("README.md")), "things go here");
}

#[test]
fn branch_pruner_builder() {
    let repo = repository_builder_branch_pruner::create_repository();
    assert_eq!(current_branch(&repo), "master");
    let parents = exec_ok(["log", "-1", "--format=%P"], repo.path()).stdout;
    assert_eq!(parents.split_whitespace().count(), 2);
    let reflog = exec(["reflog", "--all"], repo.path());
    assert_eq!(reflog.stdout, "");
    assert_eq!(read(repo.join("baz")), "very much more words");
}

#[test]
fn test_repo_exposes_paths_and_a_model() {
    let repo = setup_empty_repository();
    assert_eq!(repo.as_ref(), repo.path());
    assert_eq!(repo.temp_dir(), repo.path());
    assert_eq!(repo.join("x"), repo.path().join("x"));
    assert_eq!(repo.model().path, repo.path());
    let path = repo.path().to_path_buf();
    let clone = repo.clone();
    drop(repo);
    assert!(path.exists(), "a clone keeps the directory");
    drop(clone);
    assert!(!path.exists());
}
