//! `corvene_platform::android::Bridge` over the Kotlin callbacks: what the
//! core's Android paths ask the activity for. The first cut answers what
//! the repository list and the basics need; the rest reports "not
//! available" until the matching Kotlin side exists (M-A3).

#[cfg(target_os = "android")]
use std::path::{Path, PathBuf};
#[cfg(target_os = "android")]
use std::sync::Arc;

#[cfg(target_os = "android")]
use crate::api::{HostEvents, HostInfo};

#[cfg(target_os = "android")]
pub struct KotlinBridge {
    pub events: Arc<dyn HostEvents>,
    pub info: HostInfo,
}

#[cfg(target_os = "android")]
fn unavailable<T>(what: &str) -> Result<T, String> {
    Err(format!("{what} is not available in this build yet"))
}

#[cfg(target_os = "android")]
impl corvene_platform::android::Bridge for KotlinBridge {
    fn has_all_files_access(&self) -> bool {
        self.info.has_all_files_access
    }
    fn can_request_all_files_access(&self) -> bool {
        self.info.can_request_all_files_access
    }
    fn allows_downloaded_code(&self) -> bool {
        self.info.allows_downloaded_code
    }
    fn grammar_module_dir(&self) -> Option<PathBuf> {
        self.info.grammar_module_dir.as_ref().map(PathBuf::from)
    }
    fn install_grammar_module(&self) {}
    fn uninstall_grammar_module(&self) {}
    fn request_all_files_access(&self) {
        self.events.request_all_files_access();
    }
    fn notifications_allowed(&self) -> Option<bool> {
        self.info.notifications_allowed
    }
    fn request_notification_permission(&self) {
        self.events.request_notification_permission();
    }
    fn show_notification(&self, identifier: &str, title: &str, body: &str, payload: &str) {
        self.events.show_notification(
            identifier.to_string(),
            title.to_string(),
            body.to_string(),
            payload.to_string(),
        );
    }
    fn package_installed(&self, _package: &str) -> bool {
        false
    }
    fn view_path(&self, path: &Path) -> Result<(), String> {
        self.events
            .open_path(path.to_string_lossy().into_owned(), false);
        Ok(())
    }
    fn view_apps(&self) -> Vec<(String, String)> {
        Vec::new()
    }
    fn app_icon(&self, _key: &str) -> Option<Vec<u8>> {
        None
    }
    fn view_path_with(
        &self,
        _path: &Path,
        _component: &str,
        _line: Option<u32>,
    ) -> Result<(), String> {
        unavailable("opening in another application")
    }
    fn view_path_with_chooser(&self, path: &Path) -> Result<(), String> {
        self.view_path(path)
    }
    fn share_path(&self, _path: &Path) -> Result<(), String> {
        unavailable("sharing")
    }
    fn open_termux(&self, _dir: &Path) -> Result<(), String> {
        unavailable("Termux")
    }
    fn run_termux(&self, _program: &str, _arguments: &[String], _dir: &Path) -> Result<(), String> {
        unavailable("Termux")
    }
    fn termux_programs(&self, _candidates: &[&str]) -> Option<Vec<String>> {
        None
    }
    fn transfer_active(&self, active: bool) {
        self.events.transfer_active(active);
    }
    fn toggle_full_screen(&self) {}
    fn relaunch(&self) {
        self.events.quit();
    }
    fn bring_to_front(&self) {
        self.events.bring_to_front();
    }
    fn toast(&self, message: &str) {
        self.events.toast(message.to_string());
    }
}
