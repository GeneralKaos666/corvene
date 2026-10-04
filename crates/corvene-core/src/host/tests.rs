//! The dispatcher on a `LocalHost`: what the Android library does, without
//! a window or a toolkit.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use corvene_store::Store;

use super::local::{LocalHost, Msg, run_until_idle};
use super::{Host, StateHandle};
use crate::dispatcher::Dispatcher;
use crate::persistence::Settings;

fn git(dir: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

/// Runs the loop until `done` holds or `timeout` passes.
fn run_until(
    host: &mut LocalHost,
    rx: &std::sync::mpsc::Receiver<Msg>,
    timeout: Duration,
    mut done: impl FnMut(&LocalHost) -> bool,
) -> bool {
    let started = Instant::now();
    while started.elapsed() < timeout {
        run_until_idle(host, rx, Duration::from_millis(50));
        if done(host) {
            return true;
        }
    }
    done(host)
}

#[test]
fn init_adds_a_repository_and_loads_its_status() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["config", "commit.gpgsign", "false"]);
    std::fs::write(repo.join("README.md"), "hello\n").unwrap();
    git(&repo, &["add", "README.md"]);
    git(&repo, &["commit", "-q", "-m", "one"]);
    std::fs::write(repo.join("README.md"), "hello world\n").unwrap();

    let store = Arc::new(Store::open_in(dir.path().join("store")).unwrap());
    let (mut host, rx) = LocalHost::bare();
    let versions_before = host.version();
    Dispatcher::init(
        store,
        Settings::default(),
        Default::default(),
        Default::default(),
        &mut host,
    );
    assert!(host.has_state());
    assert!(StateHandle.read(&host).repositories.is_empty());

    Dispatcher::add_repository(repo.clone(), &mut host);
    let loaded = run_until(&mut host, &rx, Duration::from_secs(20), |host| {
        let s = host.state_ref();
        s.selected
            .and_then(|id| s.repo_states.get(&id))
            .and_then(|rs| rs.status.as_ref())
            .is_some()
    });
    assert!(loaded, "the repository's status never loaded");

    let s = host.state_ref();
    assert_eq!(s.repositories.len(), 1);
    let id = s.selected.unwrap();
    let status = s.repo_states[&id].status.as_ref().unwrap();
    assert_eq!(status.files.len(), 1, "{:?}", status.files);
    assert_eq!(status.files[0].path, "README.md");
    assert!(host.version() > versions_before, "updates notified");
}

#[test]
fn a_banner_timer_clears_on_the_loop() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(Store::open_in(dir.path().join("store")).unwrap());
    let (mut host, rx) = LocalHost::bare();
    Dispatcher::init(
        store,
        Settings::default(),
        Default::default(),
        Default::default(),
        &mut host,
    );
    run_until_idle(&mut host, &rx, Duration::from_millis(100));
    let version = host.version();
    // a state update from a main-thread future is applied and notified
    let cx: &dyn Host = &host;
    cx.spawn(async move |cx| {
        cx.background_executor()
            .timer(Duration::from_millis(10))
            .await;
        StateHandle.update(cx, |s, cx| {
            s.banner_nonce += 1;
            cx.notify();
        });
    });
    assert!(run_until(&mut host, &rx, Duration::from_secs(5), |host| {
        host.version() > version
    }));
}
