//! Corvene (`1106-apply-patch`): Repository › Apply Patch from File… and
//! Apply Patch from Clipboard read a patch, show the files it touches
//! (`Popup::ApplyPatch`, from `corvene_git::preview_patch`) and apply it:
//! as uncommitted changes (`git apply`, `--3way` when it does not apply as
//! it is) or, for a mailbox from `git format-patch`, as commits (`git am
//! --3way`). GHD writes patch files (Create Patch File, flag `821`) but
//! cannot apply one.

use std::path::PathBuf;
use std::sync::Arc;

use crate::dispatcher::Dispatcher;
use crate::host::{AsyncCtx, Host};
use crate::mco::Banner;
use crate::remote::spawn_bg;
use crate::state::Popup;

/// Patches bigger than this are refused before git sees them.
const MAX_PATCH_BYTES: u64 = 64 * 1024 * 1024;

impl Dispatcher {
    fn apply_patch_enabled(cx: &dyn Host) -> bool {
        Self::state(cx)
            .read(cx)
            .flags
            .bool(crate::flags::ids::APPLY_PATCH)
    }

    /// Repository › Apply Patch from File…: pick a `.patch` / `.diff` /
    /// mailbox file, then [`Self::preview_patch_file`].
    pub fn prompt_apply_patch_file(id: u64, cx: &mut dyn Host) {
        if !Self::apply_patch_enabled(cx) {
            return;
        }
        let receiver = cx.prompt_for_paths(crate::host::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(
                if cfg!(target_os = "macos") {
                    "Apply Patch"
                } else {
                    "Apply patch"
                }
                .into(),
            ),
        });
        cx.spawn(async move |cx: &mut AsyncCtx| {
            let picked = receiver.await.and_then(|paths| paths.into_iter().next());
            if let Some(path) = picked {
                cx.update(|cx| Self::preview_patch_file(id, path, cx));
            }
        })
        .detach();
    }

    /// Read the patch at `path` and show what it touches.
    pub fn preview_patch_file(id: u64, path: PathBuf, cx: &mut dyn Host) {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        Self::preview_patch_with(
            id,
            name,
            move || {
                let size = std::fs::metadata(&path)?.len();
                if size > MAX_PATCH_BYTES {
                    return Err(corvene_git::GitError::Gix(format!(
                        "{} is too large to be a patch Corvene can apply.",
                        path.display()
                    )));
                }
                Ok(std::fs::read(&path)?)
            },
            cx,
        );
    }

    /// Repository › Apply Patch from Clipboard: `text` is what the
    /// clipboard held.
    pub fn preview_patch_text(id: u64, text: String, cx: &mut dyn Host) {
        if !Self::apply_patch_enabled(cx) {
            return;
        }
        if text.trim().is_empty() {
            Self::show_error(
                "Could not apply the patch",
                "The clipboard holds no text. Copy a patch (the output of git diff or git \
                 format-patch) first.",
                cx,
            );
            return;
        }
        Self::preview_patch_with(id, "Clipboard".into(), move || Ok(text.into_bytes()), cx);
    }

    fn preview_patch_with(
        id: u64,
        name: String,
        read: impl FnOnce() -> corvene_git::error::Result<Vec<u8>> + Send + 'static,
        cx: &mut dyn Host,
    ) {
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        spawn_bg(
            cx,
            move || {
                let mut patch = read()?;
                // `git apply` wants the last line ended
                if patch.last().is_some_and(|b| *b != b'\n') {
                    patch.push(b'\n');
                }
                let preview = corvene_git::preview_patch(git, &workdir, &patch)?;
                Ok::<_, corvene_git::GitError>((patch, preview))
            },
            move |result, cx| match result {
                Ok((patch, preview)) => Self::show_popup(
                    Popup::ApplyPatch {
                        repo: id,
                        name,
                        patch: Arc::new(patch),
                        preview,
                    },
                    cx,
                ),
                Err(err) => Self::show_error("Could not read the patch", &err, cx),
            },
        );
    }

    /// Apply Patch › Apply: `as_commits` (a mailbox only) makes one commit
    /// per patch with `git am`; otherwise the changes are applied to the
    /// working copy, with a 3-way merge when needed. Conflicts show in the
    /// changes list like a restored stash's. `files` and `commits` are the
    /// preview's counts, for the banner.
    pub fn apply_patch(
        id: u64,
        patch: Arc<Vec<u8>>,
        files: usize,
        commits: usize,
        as_commits: bool,
        cx: &mut dyn Host,
    ) {
        Self::close_popup(cx);
        let Some((git, workdir)) = Self::repo_context(id, cx) else {
            return;
        };
        spawn_bg(
            cx,
            move || {
                if as_commits {
                    corvene_git::apply_mailbox(git, &workdir, &patch)?;
                    return Ok(Banner::PatchApplied {
                        files,
                        commits,
                        conflicts: 0,
                    });
                }
                let conflicts = match corvene_git::apply_patch(git, &workdir, &patch)? {
                    corvene_git::PatchApply::Applied => 0,
                    corvene_git::PatchApply::Conflicts(files) => files.len(),
                };
                Ok::<_, corvene_git::GitError>(Banner::PatchApplied {
                    files,
                    commits: 0,
                    conflicts,
                })
            },
            move |result, cx| {
                match result {
                    Ok(banner) => {
                        let section = if as_commits {
                            corvene_models::Section::History
                        } else {
                            corvene_models::Section::Changes
                        };
                        Self::show_section(id, section, cx);
                        Self::set_banner(banner, cx);
                    }
                    Err(err) => Self::show_error("Could not apply the patch", &err, cx),
                }
                Self::refresh_repository(id, cx);
            },
        );
    }
}
