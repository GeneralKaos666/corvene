//! Android: moving a repository from Corvene's own storage to shared storage
//! (`Popup::MoveToSharedStorage`; GitHub Desktop has no Android build, so
//! nothing here mirrors it).
//!
//! Corvene's own storage (`files/repositories`) is closed to every other
//! application. Termux, which "Open in Termux" and the Termux editors start,
//! reaches only shared storage (`/storage/emulated/0`), and only when the
//! `foss` flavour was granted "All files access". The error dialogs those
//! integrations show for a repository in Corvene's storage therefore offer
//! to move it: the folder is copied (shared storage is another filesystem,
//! a rename cannot cross), git is pointed at the copy, the repository entry
//! keeps its id, alias and settings, and the old folder is removed once the
//! copy is in use. Symbolic links become files holding the link's target,
//! which is how git itself checks them out with `core.symlinks=false`.

use std::path::{Component, Path, PathBuf};

#[cfg(target_os = "android")]
use crate::host::AsyncCtx;
use crate::host::Host;
#[cfg(target_os = "android")]
use corvene_git::CancelToken;

/// Whether shared storage can be used in place on this device and build.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SharedStorageAccess {
    /// Not Android, or a build (the `play` flavour) that cannot ask for
    /// "All files access".
    Unavailable,
    /// "All files access" may be asked for but was not granted yet.
    NeedsPermission,
    /// Granted; `root` is shared storage's root.
    Granted { root: PathBuf },
}

/// What the dialog's destination field holds, checked.
pub type SharedStorageDestination = Result<PathBuf, String>;

/// The folder under shared storage's root that holds moved repositories by
/// default (`/storage/emulated/0/Corvene/<name>`).
pub const DEFAULT_FOLDER: &str = "Corvene";

/// What the copy produced.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CopyReport {
    pub files: u64,
    /// Symbolic links written as files holding the link's target.
    pub symlinks: u64,
}

/// Checks `text` as the destination for the repository at `source`: a folder
/// on shared storage (under `root`), outside the repository, that does not
/// exist yet or is empty.
pub fn check_destination(text: &str, root: &Path, source: &Path) -> SharedStorageDestination {
    let text = text.trim();
    if text.is_empty() {
        return Err("Enter the folder the repository moves to.".to_string());
    }
    let path = Path::new(text);
    if !path.is_absolute() {
        return Err("Enter a full path.".to_string());
    }
    let path = normalize(path);
    if !path.starts_with(root) {
        return Err(format!(
            "The folder has to be on shared storage, under {}.",
            root.display()
        ));
    }
    if path == root {
        return Err("Enter a folder for the repository, not shared storage itself.".to_string());
    }
    if path.starts_with(source) {
        return Err("The new location cannot be inside the repository.".to_string());
    }
    match std::fs::read_dir(&path) {
        Ok(mut entries) => {
            if entries.next().is_some() {
                Err("That folder already exists and is not empty.".to_string())
            } else {
                Ok(path)
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(path),
        Err(err) if err.kind() == std::io::ErrorKind::NotADirectory => {
            Err("A file with that name already exists.".to_string())
        }
        Err(err) => Err(format!("That folder cannot be used: {err}")),
    }
}

/// `path` with `.` and `..` folded lexically.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Why a repository cannot be moved as a folder: its `.git` is not the
/// repository (a linked worktree), or other worktrees point into it by its
/// absolute path.
pub fn check_source(source: &Path) -> Result<(), String> {
    let git_dir = source.join(".git");
    if git_dir.is_file() {
        return Err(
            "This is a linked worktree of another repository. Move that repository instead."
                .to_string(),
        );
    }
    if !git_dir.is_dir() {
        return Err("The folder is not a Git repository.".to_string());
    }
    let has_worktrees = std::fs::read_dir(git_dir.join("worktrees"))
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false);
    if has_worktrees {
        return Err(
            "The repository has linked worktrees, which record where it is. Remove them \
             first (Branch › Worktrees)."
                .to_string(),
        );
    }
    Ok(())
}

/// Copies the folder `source` to `destination` (created if needed; an empty
/// folder is fine). Symbolic links are written as files holding their
/// target. `progress(done, total)` is called after every entry; `cancelled`
/// is checked between entries and stops the copy with
/// [`std::io::ErrorKind::Interrupted`]. Nothing is removed on failure: the
/// caller decides what to do with the partial copy.
pub fn copy_tree(
    source: &Path,
    destination: &Path,
    cancelled: &dyn Fn() -> bool,
    progress: &mut dyn FnMut(u64, u64),
) -> std::io::Result<CopyReport> {
    let total = count_entries(source)?;
    std::fs::create_dir_all(destination)?;
    let mut copy = Copy {
        cancelled,
        progress,
        done: 0,
        total,
        report: CopyReport::default(),
    };
    copy.dir(source, destination)?;
    Ok(copy.report)
}

/// Every file, folder and link below `root`.
fn count_entries(root: &Path) -> std::io::Result<u64> {
    let mut total = 0;
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            total += 1;
            if entry.file_type()?.is_dir() {
                pending.push(entry.path());
            }
        }
    }
    Ok(total)
}

struct Copy<'a> {
    cancelled: &'a dyn Fn() -> bool,
    progress: &'a mut dyn FnMut(u64, u64),
    done: u64,
    total: u64,
    report: CopyReport,
}

impl Copy<'_> {
    fn dir(&mut self, source: &Path, destination: &Path) -> std::io::Result<()> {
        for entry in std::fs::read_dir(source)? {
            let entry = entry?;
            let name = entry.file_name();
            let target = destination.join(&name);
            // the destination was empty: a name that exists already is the
            // same name in another case on a filesystem that folds case
            if std::fs::symlink_metadata(&target).is_ok() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    format!(
                        "two entries of {} differ only in case ({}); the destination does \
                         not tell them apart",
                        source.display(),
                        name.to_string_lossy()
                    ),
                ));
            }
            let kind = entry.file_type()?;
            if kind.is_dir() {
                std::fs::create_dir(&target)?;
                self.dir(&entry.path(), &target)?;
            } else if kind.is_symlink() {
                let link = std::fs::read_link(entry.path())?;
                std::fs::write(&target, link.as_os_str().as_encoded_bytes())?;
                self.report.symlinks += 1;
            } else {
                let mut from = std::fs::File::open(entry.path())?;
                let mut to = std::fs::File::create(&target)?;
                std::io::copy(&mut from, &mut to)?;
                self.report.files += 1;
            }
            self.done += 1;
            (self.progress)(self.done, self.total);
            if (self.cancelled)() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "cancelled",
                ));
            }
        }
        Ok(())
    }
}

/// Whether the filesystem holding `dir` folds case (shared storage does):
/// a probe file is looked up in another case.
pub fn folds_case(dir: &Path) -> bool {
    let probe = dir.join("corvene-case-probe");
    if std::fs::write(&probe, b"").is_err() {
        return false;
    }
    let folded = dir.join("CORVENE-CASE-PROBE").exists();
    let _ = std::fs::remove_file(&probe);
    folded
}

/// The result of the background part of a move.
#[cfg(target_os = "android")]
struct Moved {
    info: corvene_models::RepositoryInfo,
    report: CopyReport,
}

#[cfg(target_os = "android")]
fn move_failed(destination: &Path, message: String) -> String {
    if let Err(err) = std::fs::remove_dir_all(destination) {
        tracing::warn!(%err, path = %destination.display(), "partial copy left behind");
    }
    message
}

impl crate::Dispatcher {
    /// Whether `path` is on Android's shared storage (false elsewhere).
    pub fn on_shared_storage(path: &Path) -> bool {
        #[cfg(target_os = "android")]
        {
            corvene_platform::android::is_shared_storage(path)
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = path;
            false
        }
    }

    /// Whether this device and build can use shared storage in place.
    pub fn shared_storage_access() -> SharedStorageAccess {
        #[cfg(target_os = "android")]
        {
            let Some(bridge) = corvene_platform::android::bridge() else {
                return SharedStorageAccess::Unavailable;
            };
            if bridge.has_all_files_access() {
                return match bridge.shared_storage_dir() {
                    Some(root) => SharedStorageAccess::Granted { root },
                    None => SharedStorageAccess::Unavailable,
                };
            }
            if bridge.can_request_all_files_access() {
                return SharedStorageAccess::NeedsPermission;
            }
            SharedStorageAccess::Unavailable
        }
        #[cfg(not(target_os = "android"))]
        SharedStorageAccess::Unavailable
    }

    /// Android: the repository holding `path` (its root or a file in it)
    /// when it is in Corvene's own storage and this build can move it, with
    /// `then` built from `path` relative to the root. The Termux error
    /// dialogs offer the move when this is `Some`.
    pub fn shared_storage_move_for(
        path: &Path,
        then: impl FnOnce(PathBuf) -> crate::AfterSharedStorageMove,
        cx: &dyn Host,
    ) -> Option<crate::SharedStorageMove> {
        #[cfg(target_os = "android")]
        {
            if corvene_platform::android::is_shared_storage(path)
                || Self::shared_storage_access() == SharedStorageAccess::Unavailable
            {
                return None;
            }
            let s = Self::state(cx).read(cx);
            let repo = s
                .repositories
                .iter()
                .filter(|r| path.starts_with(&r.path))
                .max_by_key(|r| r.path.as_os_str().len())?;
            let relative = path.strip_prefix(&repo.path).ok()?.to_path_buf();
            Some(crate::SharedStorageMove {
                repo: repo.id,
                then: then(relative),
            })
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = (path, then, cx);
            None
        }
    }

    /// `<root>/Corvene/<folder name>` for repository `repo`.
    pub fn default_shared_storage_destination(repo: u64, cx: &dyn Host) -> Option<PathBuf> {
        let SharedStorageAccess::Granted { root } = Self::shared_storage_access() else {
            return None;
        };
        let s = Self::state(cx).read(cx);
        let path = &s.repository(repo)?.path;
        let name = path.file_name()?;
        Some(root.join(DEFAULT_FOLDER).join(name))
    }

    /// [`check_destination`] for repository `repo`.
    pub fn check_shared_storage_destination(
        repo: u64,
        text: &str,
        cx: &dyn Host,
    ) -> SharedStorageDestination {
        let SharedStorageAccess::Granted { root } = Self::shared_storage_access() else {
            return Err("Shared storage is not available.".to_string());
        };
        let s = Self::state(cx).read(cx);
        let Some(source) = s.repository(repo).map(|r| r.path.clone()) else {
            return Err("The repository is gone.".to_string());
        };
        check_destination(text, &root, &source)
    }

    /// Copies repository `repo` to `destination`, switches the entry to the
    /// copy, removes the old folder and runs `then` there. Progress is in
    /// `AppState::shared_storage_move`; a failure stays there as
    /// [`crate::SharedStorageMoveStage::Failed`] until the dialog closes.
    pub fn move_to_shared_storage(
        repo: u64,
        destination: PathBuf,
        then: crate::AfterSharedStorageMove,
        cx: &mut dyn Host,
    ) {
        #[cfg(not(target_os = "android"))]
        {
            let _ = (repo, destination, then, cx);
        }
        #[cfg(target_os = "android")]
        {
            use crate::state::{SharedStorageMoveStage, SharedStorageMoveState};
            use tracing::{info, warn};

            let state = Self::state(cx);
            let (git, source) = {
                let s = state.read(cx);
                (s.git.clone(), s.repository(repo).map(|r| r.path.clone()))
            };
            let (Some(git), Some(source)) = (git, source) else {
                return;
            };
            if let Err(message) = check_source(&source) {
                state.update(cx, |s, cx| {
                    s.shared_storage_move = Some(SharedStorageMoveState {
                        repo,
                        destination,
                        stage: SharedStorageMoveStage::Failed(message),
                        cancel: CancelToken::new(),
                    });
                    cx.notify();
                });
                return;
            }
            let cancel = CancelToken::new();
            state.update(cx, |s, cx| {
                s.shared_storage_move = Some(SharedStorageMoveState {
                    repo,
                    destination: destination.clone(),
                    stage: SharedStorageMoveStage::Copying { done: 0, total: 0 },
                    cancel: cancel.clone(),
                });
                cx.notify();
            });

            let (tx, rx) = std::sync::mpsc::channel::<SharedStorageMoveStage>();
            let copy_source = source.clone();
            let copy_destination = destination.clone();
            let copy_cancel = cancel.clone();
            let task = cx.background_executor().spawn(async move {
                let destination = copy_destination;
                let cancelled = || copy_cancel.is_cancelled();
                let mut progress = |done, total| {
                    let _ = tx.send(SharedStorageMoveStage::Copying { done, total });
                };
                let report = match copy_tree(&copy_source, &destination, &cancelled, &mut progress)
                {
                    Ok(report) => report,
                    Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {
                        return Err(move_failed(&destination, String::new()));
                    }
                    Err(err) => {
                        return Err(move_failed(
                            &destination,
                            format!("The repository could not be copied: {err}"),
                        ));
                    }
                };
                let _ = tx.send(SharedStorageMoveStage::Checking);
                crate::dispatcher::android_prepare_repository(git.clone(), &destination);
                if let Err(err) = corvene_git::refresh_index(git, &destination) {
                    warn!(%err, "index refresh after the move failed");
                }
                match corvene_git::open_repository(&destination) {
                    Ok(info) => Ok(Moved { info, report }),
                    Err(err) => Err(move_failed(
                        &destination,
                        format!("Git cannot open the copy: {err}"),
                    )),
                }
            });

            // progress pump, as for a clone
            let pump_state = state;
            cx.spawn(async move |cx: &mut AsyncCtx| {
                loop {
                    let mut latest = None;
                    while let Ok(stage) = rx.try_recv() {
                        latest = Some(stage);
                    }
                    if let Some(stage) = latest {
                        let done =
                            pump_state.update(cx, |s, cx| match s.shared_storage_move.as_mut() {
                                Some(m) if m.repo == repo && !m.stage.is_final() => {
                                    m.stage = stage;
                                    cx.notify();
                                    false
                                }
                                _ => true,
                            });
                        if done {
                            break;
                        }
                    }
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(50))
                        .await;
                    let gone = pump_state.read_with(cx, |s, _| {
                        s.shared_storage_move
                            .as_ref()
                            .is_none_or(|m| m.repo != repo || m.stage.is_final())
                    });
                    if gone {
                        break;
                    }
                }
            })
            .detach();

            cx.spawn(async move |cx: &mut AsyncCtx| {
                let result = task.await;
                cx.update(|cx| match result {
                    Ok(moved) => {
                        info!(
                            from = %source.display(),
                            to = %destination.display(),
                            files = moved.report.files,
                            symlinks = moved.report.symlinks,
                            "repository moved to shared storage"
                        );
                        Self::finish_shared_storage_move(
                            repo,
                            source,
                            destination,
                            moved,
                            then,
                            cx,
                        );
                    }
                    Err(message) if message.is_empty() => {
                        info!("move to shared storage cancelled");
                        Self::state(cx).update(cx, |s, cx| {
                            if s.shared_storage_move
                                .as_ref()
                                .is_some_and(|m| m.repo == repo)
                            {
                                s.shared_storage_move = None;
                            }
                            cx.notify();
                        });
                    }
                    Err(message) => {
                        warn!(%message, "move to shared storage failed");
                        Self::state(cx).update(cx, |s, cx| {
                            if let Some(m) =
                                s.shared_storage_move.as_mut().filter(|m| m.repo == repo)
                            {
                                m.stage = SharedStorageMoveStage::Failed(message);
                            }
                            cx.notify();
                        });
                    }
                });
            })
            .detach();
        }
    }

    /// The copy is complete and git opened it: the entry points there, the
    /// dialog closes, `then` runs and the old folder goes away in the
    /// background.
    #[cfg(target_os = "android")]
    fn finish_shared_storage_move(
        repo: u64,
        source: PathBuf,
        destination: PathBuf,
        moved: Moved,
        then: crate::AfterSharedStorageMove,
        cx: &mut dyn Host,
    ) {
        use crate::state::Popup;

        let state = Self::state(cx);
        state.update(cx, |s, cx| {
            if let Some(r) = s.repositories.iter_mut().find(|r| r.id == repo) {
                r.path = destination.clone();
                r.missing = false;
                if r.main_worktree_path.as_deref() == Some(source.as_path()) {
                    r.main_worktree_path = Some(destination.clone());
                }
            }
            let repo_state = s.repo_state_mut(repo);
            repo_state.info = Some(moved.info);
            repo_state.unsafe_path = None;
            repo_state.error = None;
            if s.watched_repo == Some(repo) {
                s.watcher = None;
                s.watched_repo = None;
            }
            s.shared_storage_move = None;
            let finished: Vec<crate::popup_manager::StackedPopup> = s
                .popups
                .all_popups()
                .iter()
                .filter(
                    |p| matches!(p.popup, Popup::MoveToSharedStorage { repo: r, .. } if r == repo),
                )
                .cloned()
                .collect();
            for popup in finished {
                s.popups.remove_popup(popup);
            }
            crate::dispatcher::persist_repositories(s);
            cx.notify();
        });
        Self::select_repository(repo, cx);
        match then {
            crate::AfterSharedStorageMove::Nothing => {}
            crate::AfterSharedStorageMove::OpenShell => Self::open_in_shell(&destination, cx),
            crate::AfterSharedStorageMove::OpenEditor { relative, line } => {
                Self::open_in_editor_at(destination.join(relative), line, cx)
            }
        }
        crate::remote::spawn_bg(
            cx,
            move || std::fs::remove_dir_all(&source).map_err(|err| (source, err)),
            |result, cx| {
                if let Err((source, err)) = result {
                    tracing::warn!(%err, path = %source.display(), "old copy not removed");
                    Self::show_error(
                        "Could not remove the old copy",
                        format!(
                            "The repository is now on shared storage, but its old folder {} \
                             could not be removed: {err}",
                            source.display()
                        ),
                        cx,
                    );
                }
            },
        );
    }

    /// The dialog's Cancel while copying: the partial copy is removed.
    pub fn cancel_shared_storage_move(cx: &mut dyn Host) {
        if let Some(m) = Self::state(cx).read(cx).shared_storage_move.as_ref() {
            m.cancel.cancel();
        }
    }

    /// The dialog closed: a failed move's message is dropped (a move in
    /// progress goes on and shows again when the dialog reopens).
    pub fn dismiss_shared_storage_move(cx: &mut dyn Host) {
        Self::state(cx).update(cx, |s, cx| {
            if s.shared_storage_move
                .as_ref()
                .is_some_and(|m| matches!(m.stage, crate::SharedStorageMoveStage::Failed(_)))
            {
                s.shared_storage_move = None;
                cx.notify();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "corvene-shared-storage-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    // `report` is read on unix only (the symlink count)
    #[cfg_attr(not(unix), allow(unused_variables))]
    fn copies_files_folders_and_links_as_text() {
        let root = temp_dir("copy");
        let source = root.join("src");
        std::fs::create_dir_all(source.join(".git/objects")).unwrap();
        std::fs::write(source.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        std::fs::write(source.join("a.txt"), "a").unwrap();
        std::fs::create_dir(source.join("dir")).unwrap();
        std::fs::write(source.join("dir/b.txt"), "bb").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("dir/b.txt", source.join("link")).unwrap();
        let destination = root.join("dst");
        let mut seen = Vec::new();
        let report = copy_tree(&source, &destination, &|| false, &mut |done, total| {
            seen.push((done, total))
        })
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(destination.join(".git/HEAD")).unwrap(),
            "ref: refs/heads/main\n"
        );
        assert_eq!(
            std::fs::read_to_string(destination.join("dir/b.txt")).unwrap(),
            "bb"
        );
        assert!(destination.join(".git/objects").is_dir());
        #[cfg(unix)]
        {
            assert_eq!(report.symlinks, 1);
            assert_eq!(
                std::fs::read_to_string(destination.join("link")).unwrap(),
                "dir/b.txt"
            );
            assert!(!destination.join("link").is_symlink());
            assert_eq!(report.files, 3);
        }
        let total = seen.last().unwrap().1;
        assert_eq!(seen.last().unwrap().0, total);
        assert!(seen.iter().all(|(done, t)| *t == total && *done <= total));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn cancel_stops_the_copy() {
        let root = temp_dir("cancel");
        let source = root.join("src");
        std::fs::create_dir_all(&source).unwrap();
        for n in 0..5 {
            std::fs::write(source.join(format!("{n}.txt")), "x").unwrap();
        }
        let destination = root.join("dst");
        let err = copy_tree(&source, &destination, &|| true, &mut |_, _| {}).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::Interrupted);
        assert_eq!(std::fs::read_dir(&destination).unwrap().count(), 1);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn destination_rules() {
        let root = temp_dir("dest");
        let shared = root.join("storage/emulated/0");
        let source = root.join("data/repositories/repo");
        std::fs::create_dir_all(shared.join("taken")).unwrap();
        std::fs::write(shared.join("taken/file"), "").unwrap();
        std::fs::create_dir_all(shared.join("empty")).unwrap();
        std::fs::create_dir_all(&source).unwrap();
        let check = |text: &str| check_destination(text, &shared, &source);
        assert!(check("").is_err());
        assert!(check("relative/path").is_err());
        assert!(check(&root.join("elsewhere").display().to_string()).is_err());
        assert!(check(&shared.display().to_string()).is_err());
        assert!(check(&source.join("inner").display().to_string()).is_err());
        assert!(check(&shared.join("taken").display().to_string()).is_err());
        assert!(check(&shared.join("taken/file").display().to_string()).is_err());
        assert_eq!(
            check(&shared.join("empty").display().to_string()),
            Ok(shared.join("empty"))
        );
        assert_eq!(
            check(&format!("{}/Corvene/./x/../repo ", shared.display())),
            Ok(shared.join("Corvene/repo"))
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn source_rules() {
        let root = temp_dir("source");
        let plain = root.join("plain");
        std::fs::create_dir_all(plain.join(".git")).unwrap();
        assert_eq!(check_source(&plain), Ok(()));
        let linked = root.join("linked");
        std::fs::create_dir_all(&linked).unwrap();
        std::fs::write(linked.join(".git"), "gitdir: /elsewhere").unwrap();
        assert!(check_source(&linked).is_err());
        let with_worktrees = root.join("with");
        std::fs::create_dir_all(with_worktrees.join(".git/worktrees/one")).unwrap();
        assert!(check_source(&with_worktrees).is_err());
        assert!(check_source(&root.join("none")).is_err());
        let _ = std::fs::remove_dir_all(root);
    }
}
