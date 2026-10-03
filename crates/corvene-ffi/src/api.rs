//! The Kotlin-visible interface.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use corvene_core::Dispatcher;
use corvene_core::host::{Host, LoopHandle, spawn_loop};
use corvene_core::persistence::StoreExt;

use crate::runtime::Services;
use crate::vm::{RepoListVm, repo_list};

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
}
