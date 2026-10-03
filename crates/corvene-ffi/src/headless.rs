//! The background fetch without the app: WorkManager starts the process
//! for `CorveneFetchWorker`; with no `Corvene` alive it calls
//! [`headless_fetch`], which sets up what the constructor would (the
//! environment, the askpass socket) and runs
//! `corvene_core::headless::background_fetch`. Should the app start
//! meanwhile, it calls [`end_headless`] first: git is stopped and the
//! fetch returns.

use std::sync::Mutex;

use corvene_core::headless::{CancelToken, Outcome};

static CANCEL: Mutex<Option<CancelToken>> = Mutex::new(None);

/// What a headless fetch did.
#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeadlessOutcome {
    Fetched,
    Skipped,
    Failed,
}

/// Blocking: runs the due fetch of the selected repository. The Java
/// context must be attached (`NativeContext.attach`) for the Keystore.
#[uniffi::export]
pub fn headless_fetch(files_dir: String) -> HeadlessOutcome {
    let files = std::path::PathBuf::from(files_dir);
    corvene_platform::android::prepare_environment(&files);
    crate::runtime::init_logging();
    if !crate::jni::attached() {
        tracing::warn!("NativeContext.attach was not called: the Keystore is unavailable");
    }
    let cancel = CancelToken::new();
    if let Ok(mut slot) = CANCEL.lock() {
        *slot = Some(cancel.clone());
    }
    corvene_core::headless::set_cancel_token(Some(cancel));
    corvene_core::askpass::serve_socket();
    // `paths::app_support_dir` follows HOME, which is set now
    let outcome =
        corvene_core::headless::background_fetch(&corvene_platform::paths::app_support_dir());
    match &outcome {
        Ok(outcome) => tracing::info!("headless background fetch: {outcome:?}"),
        Err(err) => tracing::warn!("headless background fetch failed: {err}"),
    }
    corvene_core::headless::set_cancel_token(None);
    if let Ok(mut slot) = CANCEL.lock() {
        *slot = None;
    }
    match outcome {
        Ok(Outcome::Fetched) => HeadlessOutcome::Fetched,
        Ok(Outcome::Skipped(_)) => HeadlessOutcome::Skipped,
        Err(_) => HeadlessOutcome::Failed,
    }
}

/// Stops a running headless fetch (the app is starting).
#[uniffi::export]
pub fn end_headless() {
    if let Ok(slot) = CANCEL.lock()
        && let Some(cancel) = slot.as_ref()
    {
        cancel.cancel();
    }
}
