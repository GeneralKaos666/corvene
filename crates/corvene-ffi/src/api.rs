//! The Kotlin-visible interface.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use corvene_core::Dispatcher;
use corvene_core::host::{Host, LoopHandle, spawn_loop};
use corvene_core::persistence::{StoreExt, UncommittedChangesStrategy};

use crate::runtime::Services;
use crate::vm::{
    BranchesVm, ChangesVm, CommitDetailVm, DiffHeaderVm, DiffRowVm, HistoryVm, RepoListVm,
    branches, changes, commit_detail, diff_header, diff_rows, history, repo_list,
};

/// What the engine asks of the Android side. Called on the engine's
/// threads: post to the main looper before touching views.
#[uniffi::export(with_foreign)]
pub trait HostEvents: Send + Sync {
    /// The state changed; screens re-query. Coalesced per run-loop turn.
    fn state_changed(&self, version: u64);
    /// Open in the browser (Custom Tabs for the sign-in URLs).
    fn open_url(&self, url: String);
    /// A folder or file picker; answered with `Corvene::paths_picked`.
    fn pick_paths(&self, request: u64, directories: bool, multiple: bool, prompt: Option<String>);
    fn write_clipboard(&self, text: String);
    /// Open a file with another application, or (`reveal`) show its folder.
    fn open_path(&self, path: String, reveal: bool);
    fn toast(&self, message: String);
    fn show_notification(&self, identifier: String, title: String, body: String, payload: String);
    fn request_notification_permission(&self);
    fn request_all_files_access(&self);
    /// A network operation started or ended (foreground service).
    fn transfer_active(&self, active: bool);
    fn bring_to_front(&self);
    /// The engine wants the process to end (relaunch after a flag change).
    fn quit(&self);
}

/// What the Android side knows at start-up that the engine asks for later.
#[derive(uniffi::Record, Clone, Debug, Default)]
pub struct HostInfo {
    pub has_all_files_access: bool,
    pub can_request_all_files_access: bool,
    pub allows_downloaded_code: bool,
    pub grammar_module_dir: Option<String>,
    pub notifications_allowed: Option<bool>,
}

#[derive(uniffi::Error, thiserror::Error, Debug)]
pub enum CoreError {
    #[error("{message}")]
    Failed { message: String },
}

/// The engine. One per process.
#[derive(uniffi::Object)]
pub struct Corvene {
    loop_: LoopHandle,
    services: Arc<Services>,
}

#[uniffi::export]
impl Corvene {
    /// Starts the engine: the process environment (`HOME`, the bundled
    /// git), the store under `files_dir`, the dispatcher on its own thread.
    /// `env` sets `CORVENE_*` variables (`CORVENE_FLAGS`, `CORVENE_LOG`)
    /// before anything reads them.
    #[uniffi::constructor]
    pub fn new(
        files_dir: String,
        env: HashMap<String, String>,
        info: HostInfo,
        events: Arc<dyn HostEvents>,
    ) -> Result<Arc<Self>, CoreError> {
        for (key, value) in &env {
            if key.starts_with("CORVENE_") {
                // SAFETY: before the engine spawns any thread
                unsafe { std::env::set_var(key, value) };
            }
        }
        let files = PathBuf::from(files_dir);
        #[cfg(target_os = "android")]
        {
            corvene_platform::android::prepare_environment(&files);
            corvene_platform::android::set_bridge(crate::bridge::KotlinBridge {
                events: events.clone(),
                info: info.clone(),
            });
        }
        #[cfg(not(target_os = "android"))]
        let _ = (&files, &info);
        crate::runtime::init_logging();
        Dispatcher::prefetch_git();
        #[cfg(target_os = "android")]
        corvene_core::askpass::serve_socket();

        let services = Arc::new(Services::new(events.clone()));
        let on_changed = {
            let events = events.clone();
            move |version| events.state_changed(version)
        };
        let loop_ = spawn_loop("corvene-main", services.clone(), on_changed).map_err(|err| {
            CoreError::Failed {
                message: err.to_string(),
            }
        })?;
        let init = loop_.query_blocking(|host| -> Result<(), String> {
            let dir = corvene_platform::paths::app_support_dir();
            let store = corvene_store::Store::open_in(&dir).map_err(|err| err.to_string())?;
            let store = Arc::new(store);
            let settings = store.settings().unwrap_or_default();
            let flag_overrides = store.flags().unwrap_or_default();
            let (flags_env, flag_errors) = corvene_core::flags::env::from_env();
            for err in &flag_errors {
                tracing::warn!("{err}");
            }
            Dispatcher::init(store, settings, flag_overrides, flags_env, host);
            Ok(())
        });
        match init {
            Some(Ok(())) => {}
            Some(Err(message)) => return Err(CoreError::Failed { message }),
            None => {
                return Err(CoreError::Failed {
                    message: "the engine thread ended during start-up".into(),
                });
            }
        }
        Ok(Arc::new(Corvene { loop_, services }))
    }

    /// Kotlin's answer to `HostEvents::pick_paths` (`None` = cancelled).
    pub fn paths_picked(&self, request: u64, paths: Option<Vec<String>>) {
        self.services.paths_picked(request, paths);
    }

    /// An `x-corvene://` URL or a GitHub URL the system handed over.
    pub fn app_url(&self, url: String) {
        self.loop_
            .post(move |host| Dispatcher::handle_app_url(&url, host));
    }

    /// The activity came to the front (GHD refreshes on window focus).
    pub fn focus(&self) {
        self.loop_.post(|host| Dispatcher::refresh_selected(host));
    }

    pub fn select_repository(&self, id: u64) {
        self.loop_
            .post(move |host| Dispatcher::select_repository(id, host));
    }

    pub fn remove_repository(&self, id: u64) {
        self.loop_
            .post(move |host| Dispatcher::remove_repository(id, host));
    }

    pub fn add_repository(&self, path: String) {
        self.loop_
            .post(move |host| Dispatcher::add_repository(PathBuf::from(path), host));
    }

    pub fn refresh_indicators(&self) {
        self.loop_.post(|host| Dispatcher::refresh_indicators(host));
    }

    /// Bumps after every state change (the number `state_changed` sent).
    pub async fn version(&self) -> u64 {
        self.loop_.query(|host| host.version()).await
    }

    pub async fn repo_list(&self) -> RepoListVm {
        self.loop_.query(|host| repo_list(host.state_ref())).await
    }

    // ---- the Changes tab ----

    pub fn select_section(&self, repo: u64, history: bool) {
        self.loop_.post(move |host| {
            let section = if history {
                corvene_models::Section::History
            } else {
                corvene_models::Section::Changes
            };
            Dispatcher::show_section(repo, section, host)
        });
    }

    pub fn select_file(&self, repo: u64, path: String) {
        self.loop_
            .post(move |host| Dispatcher::select_file(repo, path, host));
    }

    pub fn toggle_file_included(&self, repo: u64, path: String) {
        self.loop_
            .post(move |host| Dispatcher::toggle_file_included(repo, path, host));
    }

    /// Toggles one add/delete line of `path` in or out of the next commit.
    pub fn toggle_diff_line(&self, repo: u64, path: String, line: u32) {
        self.loop_
            .post(move |host| Dispatcher::toggle_diff_line(repo, path, line, host));
    }

    pub fn commit(&self, repo: u64, summary: String, description: String) {
        self.loop_
            .post(move |host| Dispatcher::commit(repo, summary, description, host));
    }

    pub fn undo_commit(&self, repo: u64) {
        self.loop_
            .post(move |host| Dispatcher::undo_commit(repo, host));
    }

    pub fn discard_changes(&self, repo: u64, paths: Vec<String>) {
        self.loop_
            .post(move |host| Dispatcher::discard_changes(repo, paths, host));
    }

    pub fn refresh_repository(&self, repo: u64) {
        self.loop_
            .post(move |host| Dispatcher::refresh_repository(repo, host));
    }

    pub async fn changes(&self, repo: u64) -> Option<ChangesVm> {
        self.loop_
            .query(move |host| changes(host.state_ref(), repo))
            .await
    }

    pub async fn diff_header(&self, repo: u64) -> Option<DiffHeaderVm> {
        self.loop_
            .query(move |host| diff_header(host.state_ref(), repo))
            .await
    }

    pub async fn diff_rows(
        &self,
        repo: u64,
        generation: u64,
        start: u32,
        count: u32,
    ) -> Vec<DiffRowVm> {
        self.loop_
            .query(move |host| diff_rows(host.state_ref(), repo, generation, start, count))
            .await
    }

    // ---- the History tab ----

    pub fn load_more_commits(&self, repo: u64) {
        self.loop_
            .post(move |host| Dispatcher::load_commits(repo, true, host));
    }

    pub fn select_commit(&self, repo: u64, sha: String) {
        self.loop_
            .post(move |host| Dispatcher::select_commit(repo, sha, host));
    }

    pub fn select_commits(&self, repo: u64, shas: Vec<String>) {
        self.loop_
            .post(move |host| Dispatcher::select_commits(repo, shas, host));
    }

    pub fn select_commit_file(&self, repo: u64, path: String) {
        self.loop_
            .post(move |host| Dispatcher::select_commit_file(repo, path, host));
    }

    pub async fn history(&self, repo: u64, start: u32, count: u32) -> Option<HistoryVm> {
        self.loop_
            .query(move |host| history(host.state_ref(), repo, start, count))
            .await
    }

    pub async fn commit_detail(&self, repo: u64) -> Option<CommitDetailVm> {
        self.loop_
            .query(move |host| commit_detail(host.state_ref(), repo))
            .await
    }

    // ---- branches and the sync button ----

    /// `strategy`: `None` asks when there are uncommitted changes (GHD's
    /// dialog), `"stash"` or `"move"` decides.
    pub fn checkout_branch(&self, repo: u64, name: String, strategy: Option<String>) {
        let explicit = match strategy.as_deref() {
            Some("stash") => Some(UncommittedChangesStrategy::StashOnCurrentBranch),
            Some("move") => Some(UncommittedChangesStrategy::MoveToNewBranch),
            _ => None,
        };
        self.loop_
            .post(move |host| Dispatcher::checkout_branch(repo, name, explicit, host));
    }

    pub fn create_branch(&self, repo: u64, name: String, start_point: Option<String>) {
        self.loop_
            .post(move |host| Dispatcher::create_branch(repo, name, start_point, false, host));
    }

    pub fn rename_branch(&self, repo: u64, old: String, new: String) {
        self.loop_
            .post(move |host| Dispatcher::rename_branch(repo, old, new, host));
    }

    pub fn delete_branch(&self, repo: u64, name: String, include_remote: bool) {
        self.loop_
            .post(move |host| Dispatcher::delete_branch(repo, name, include_remote, host));
    }

    pub fn fetch(&self, repo: u64) {
        self.loop_
            .post(move |host| Dispatcher::fetch(repo, false, host));
    }

    pub fn pull(&self, repo: u64) {
        self.loop_.post(move |host| Dispatcher::pull(repo, host));
    }

    pub fn push(&self, repo: u64, force_with_lease: bool) {
        self.loop_
            .post(move |host| Dispatcher::push(repo, force_with_lease, None, host));
    }

    pub async fn branches(&self, repo: u64) -> Option<BranchesVm> {
        self.loop_
            .query(move |host| branches(host.state_ref(), repo))
            .await
    }
}
