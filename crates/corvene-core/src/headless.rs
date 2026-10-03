//! The background fetch without the application: Android's WorkManager
//! starts the process about once an hour (`CorveneFetchWorker`), and when no
//! activity lives in it there is no `Dispatcher` to ask. This does what
//! `Dispatcher::background_fetch_tick` does, straight from the store: fetch
//! the selected repository when its last fetch is an hour old, with the
//! signed-in accounts' credentials (git asks through the askpass helper).
//!
//! GHD has no equivalent: its background fetcher lives in the renderer.

use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

use corvene_git::AskpassEnv;
use corvene_store::Store;

use crate::persistence::StoreExt;

pub use corvene_git::CancelToken;

/// Lets the caller stop the fetch [`background_fetch`] runs (the token is
/// cancelled when the application is opened).
pub fn set_cancel_token(token: Option<CancelToken>) {
    corvene_git::process::set_default_cancel_token(token);
}

/// What [`background_fetch`] did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Fetched,
    /// Nothing to do, and why.
    Skipped(&'static str),
}

/// Fetches the selected repository of the store in `store_dir` when it is
/// due. Blocking; the store is closed again before this returns.
pub fn background_fetch(store_dir: &Path) -> Result<Outcome, String> {
    let store = Store::open_in(store_dir).map_err(|err| err.to_string())?;
    let (env, _) = crate::flags::env::from_env();
    let flags = crate::Flags::resolve(&store.flags().unwrap_or_default(), &env);
    let mode = flags.text(crate::flags::ids::BACKGROUND_FETCH).to_string();
    if mode == "off" {
        return Ok(Outcome::Skipped("background fetch is off"));
    }
    let Some(id) = store.selected_repository().map_err(|err| err.to_string())? else {
        return Ok(Outcome::Skipped("no repository is selected"));
    };
    let repositories = store.repositories().map_err(|err| err.to_string())?;
    let Some(repository) = repositories.iter().find(|r| r.id == id) else {
        return Ok(Outcome::Skipped("the selected repository is gone"));
    };
    // GHD fetches GitHub repositories only (`244-background-fetch`)
    if mode != "any" && repository.github.is_none() {
        return Ok(Outcome::Skipped("not a GitHub repository"));
    }
    let mut logins: Vec<String> = store
        .accounts()
        .unwrap_or_default()
        .iter()
        .map(|account| format!("{}={}", account.host(), account.login))
        .collect();
    logins.extend(
        store
            .generic_logins()
            .unwrap_or_default()
            .iter()
            .map(|(host, user)| format!("{host}={user}")),
    );
    let path = repository.path.clone();
    // the application may be opened while git runs: it needs the store
    drop(store);
    let shared = flags.bool(crate::flags::ids::WORKTREE_SHARED_LAST_FETCHED);
    fetch_if_due(&path, logins.join(";"), shared)
}

fn fetch_if_due(workdir: &Path, logins: String, shared: bool) -> Result<Outcome, String> {
    if !workdir.join(".git").exists() {
        return Ok(Outcome::Skipped("the repository is missing"));
    }
    let last =
        corvene_git::last_fetched(workdir, shared).or_else(|| corvene_git::cloned_at(workdir));
    let due = last.is_none_or(|at| {
        SystemTime::now()
            .duration_since(at)
            .map(|age| age >= crate::remote::BACKGROUND_FETCH_INTERVAL)
            .unwrap_or(true)
    });
    if !due {
        return Ok(Outcome::Skipped("fetched less than an hour ago"));
    }
    let git = Arc::new(corvene_git::find_git().map_err(|err| err.to_string())?);
    let remotes = corvene_git::get_remotes(git.clone(), workdir).map_err(|err| err.to_string())?;
    let Some(remote) = corvene_git::find_default_remote(&remotes) else {
        return Ok(Outcome::Skipped("no remote"));
    };
    let askpass = AskpassEnv::current_exe(logins);
    corvene_git::fetch(git, workdir, &remote.name, askpass.as_ref(), &mut |_, _| {})
        .map_err(|err| err.to_string())?;
    Ok(Outcome::Fetched)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn fetches_a_repository_that_never_fetched() {
        let dir = tempfile::tempdir().unwrap();
        let (origin, work) = (dir.path().join("origin"), dir.path().join("work"));
        std::fs::create_dir_all(&origin).unwrap();
        std::fs::create_dir_all(&work).unwrap();
        git(&origin, &["init", "-q", "-b", "main"]);
        git(&origin, &["commit", "-q", "--allow-empty", "-m", "one"]);
        git(&work, &["init", "-q", "-b", "main"]);
        git(
            &work,
            &["remote", "add", "origin", origin.to_str().unwrap()],
        );

        assert_eq!(
            fetch_if_due(&work, String::new(), true),
            Ok(Outcome::Fetched)
        );
        assert!(work.join(".git/refs/remotes/origin/main").exists());
        // FETCH_HEAD is fresh now
        assert_eq!(
            fetch_if_due(&work, String::new(), true),
            Ok(Outcome::Skipped("fetched less than an hour ago"))
        );
    }

    #[test]
    fn skips_without_a_selected_repository() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            background_fetch(dir.path()),
            Ok(Outcome::Skipped("no repository is selected"))
        );
    }
}
