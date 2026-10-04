//! What git's output means: dugite's `GitError` table (`lib/errors.ts`,
//! `GitErrorRegexes`, in its order) and GHD's plain-language descriptions for
//! them (`lib/git/core.ts` `getDescriptionForError`), plus the pieces the
//! error dialog shows for a failed command ([`GitFailure`]).
//!
//! The patterns are dugite's regular expressions reduced to ordered literal
//! fragments: a pattern matches when every fragment is found after the one
//! before it, which is what the `(.+)` wildcards between them allow.

use std::path::{Path, PathBuf};

use crate::error::GitError;

/// dugite's `GitError`: the git failures GHD recognises in stderr.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[allow(clippy::enum_variant_names)]
pub enum KnownGitError {
    SSHKeyAuditUnverified,
    HTTPSAuthenticationFailed,
    SSHAuthenticationFailed,
    SSHPermissionDenied,
    RemoteDisconnection,
    HostDown,
    RebaseConflicts,
    MergeConflicts,
    HTTPSRepositoryNotFound,
    SSHRepositoryNotFound,
    PushNotFastForward,
    BranchDeletionFailed,
    DefaultBranchDeletionFailed,
    RevertConflicts,
    EmptyRebasePatch,
    NoMatchingRemoteBranch,
    NoExistingRemoteBranch,
    NothingToCommit,
    NoSubmoduleMapping,
    SubmoduleRepositoryDoesNotExist,
    InvalidSubmoduleSHA,
    LocalPermissionDenied,
    InvalidMerge,
    InvalidRebase,
    NonFastForwardMergeIntoEmptyHead,
    PatchDoesNotApply,
    BranchAlreadyExists,
    BadRevision,
    NotAGitRepository,
    CannotMergeUnrelatedHistories,
    LFSAttributeDoesNotMatch,
    BranchRenameFailed,
    PathDoesNotExist,
    InvalidObjectName,
    OutsideRepository,
    LockFileAlreadyExists,
    NoMergeToAbort,
    LocalChangesOverwritten,
    UnresolvedConflicts,
    ConfigLockFileAlreadyExists,
    RemoteAlreadyExists,
    TagAlreadyExists,
    MergeWithLocalChanges,
    RebaseWithLocalChanges,
    MergeCommitNoMainlineOption,
    UnsafeDirectory,
    PathExistsButNotInRef,
    PushWithFileSizeExceedingLimit,
    HexBranchNameRejected,
    ForcePushRejected,
    InvalidRefLength,
    ProtectedBranchRequiresReview,
    ProtectedBranchForcePush,
    ProtectedBranchDeleteRejected,
    ProtectedBranchRequiredStatus,
    PushWithPrivateEmail,
    PushWithSecretDetected,
    GPGFailedToSignData,
    ConflictModifyDeletedInBranch,
}

use KnownGitError::*;

/// dugite `GitErrorRegexes`, in order: the first matching entry wins.
const PATTERNS: &[(&[&str], KnownGitError)] = &[
    (
        &[
            "[EPOLICYKEYAGE]",
            "fatal: Could not read from remote repository.",
        ],
        SSHKeyAuditUnverified,
    ),
    (
        &["fatal: Authentication failed for 'https://"],
        HTTPSAuthenticationFailed,
    ),
    (&["fatal: Authentication failed"], SSHAuthenticationFailed),
    (
        &["fatal: Could not read from remote repository."],
        SSHPermissionDenied,
    ),
    (
        &["The requested URL returned error: 403"],
        HTTPSAuthenticationFailed,
    ),
    (
        &["fatal: The remote end hung up unexpectedly"],
        RemoteDisconnection,
    ),
    (
        &["fatal: the remote end hung up unexpectedly"],
        RemoteDisconnection,
    ),
    (
        &[
            "fatal: unable to access '",
            "': Failed to connect to ",
            ": Host is down",
        ],
        HostDown,
    ),
    (
        &[
            "Cloning into '",
            "'...\nfatal: unable to access '",
            "': Could not resolve host: ",
        ],
        HostDown,
    ),
    (
        &["Resolve all conflicts manually, mark them as resolved with"],
        RebaseConflicts,
    ),
    (&["Merge conflict"], MergeConflicts),
    (
        &["Automatic merge failed; fix conflicts and then commit the result"],
        MergeConflicts,
    ),
    (
        &["fatal: repository '", "' not found"],
        HTTPSRepositoryNotFound,
    ),
    (&["ERROR: Repository not found"], SSHRepositoryNotFound),
    (
        &[
            "(non-fast-forward)",
            "\nerror: failed to push some refs to '",
        ],
        PushNotFastForward,
    ),
    (
        &["(fetch first)", "\nerror: failed to push some refs to '"],
        PushNotFastForward,
    ),
    (
        &["error: unable to delete '", "': remote ref does not exist"],
        BranchDeletionFailed,
    ),
    (
        &[
            "[remote rejected] ",
            " (deletion of the current branch prohibited)",
        ],
        DefaultBranchDeletionFailed,
    ),
    (
        &[
            "error: could not revert ",
            "\nhint: after resolving the conflicts, mark the corrected paths\n\
             hint: with 'git add <paths>' or 'git rm <paths>'\n\
             hint: and commit the result with 'git commit'",
        ],
        RevertConflicts,
    ),
    (
        &[
            "Applying: ",
            "\nNo changes - did you forget to use 'git add'?\n\
             If there is nothing left to stage, chances are that something else\n",
        ],
        EmptyRebasePatch,
    ),
    (
        &[
            "There are no candidates for ",
            " among the refs that you just fetched.\n\
             Generally this means that you provided a wildcard refspec which had no\n\
             matches on the remote end.",
        ],
        NoMatchingRemoteBranch,
    ),
    (
        &[
            "Your configuration specifies to merge with the ref '",
            "'\nfrom the remote, but no such ref was fetched.",
        ],
        NoExistingRemoteBranch,
    ),
    (&["nothing to commit"], NothingToCommit),
    (
        &["o submodule mapping found in .gitmodules for path '"],
        NoSubmoduleMapping,
    ),
    (
        &[
            "fatal: repository '",
            "' does not exist\nfatal: clone of '",
            "' into submodule path '",
            "' failed",
        ],
        SubmoduleRepositoryDoesNotExist,
    ),
    (
        &[
            "Fetched in submodule path '",
            "', but it did not contain ",
            ". Direct fetching of that commit failed.",
        ],
        InvalidSubmoduleSHA,
    ),
    (
        &[
            "fatal: could not create work tree dir '",
            "'",
            ": Permission denied",
        ],
        LocalPermissionDenied,
    ),
    (&["merge: ", " - not something we can merge"], InvalidMerge),
    (&["invalid upstream "], InvalidRebase),
    (
        &["fatal: Non-fast-forward commit does not make sense into an empty head"],
        NonFastForwardMergeIntoEmptyHead,
    ),
    (&["error: ", ": patch does not apply"], PatchDoesNotApply),
    (
        &["error: ", ": already exists in working directory"],
        PatchDoesNotApply,
    ),
    (
        &["fatal: A branch named '", "' already exists"],
        BranchAlreadyExists,
    ),
    (
        &["fatal: a branch named '", "' already exists"],
        BranchAlreadyExists,
    ),
    (&["fatal: bad revision '"], BadRevision),
    (
        &["fatal: Not a git repository (or any of the parent directories): "],
        NotAGitRepository,
    ),
    (
        &["fatal: not a git repository (or any of the parent directories): "],
        NotAGitRepository,
    ),
    (
        &["fatal: refusing to merge unrelated histories"],
        CannotMergeUnrelatedHistories,
    ),
    (
        &["The ", " attribute should be ", " but is "],
        LFSAttributeDoesNotMatch,
    ),
    (&["fatal: Branch rename failed"], BranchRenameFailed),
    (&["fatal: path '", "' does not exist "], PathDoesNotExist),
    (&["fatal: invalid object name '", "'."], InvalidObjectName),
    (
        &["fatal: ", ": '", "' is outside repository"],
        OutsideRepository,
    ),
    (
        &["Another git process seems to be running in this repository, e.g."],
        LockFileAlreadyExists,
    ),
    (&["fatal: There is no merge to abort"], NoMergeToAbort),
    (
        &["error: Your local changes to the following files would be overwritten by checkout:"],
        LocalChangesOverwritten,
    ),
    (
        &["error: The following untracked working tree files would be overwritten by checkout:"],
        LocalChangesOverwritten,
    ),
    (
        &["You must edit all merge conflicts and then\n\
             mark them as resolved using git add"],
        UnresolvedConflicts,
    ),
    (
        &["fatal: Exiting because of an unresolved conflict"],
        UnresolvedConflicts,
    ),
    (
        &["error: could not lock config file ", ": File exists"],
        ConfigLockFileAlreadyExists,
    ),
    (&["error: remote ", " already exists."], RemoteAlreadyExists),
    (&["fatal: tag '", "' already exists"], TagAlreadyExists),
    (
        &["error: Your local changes to the following files would be overwritten by merge:\n"],
        MergeWithLocalChanges,
    ),
    (
        &[
            "error: cannot pull with rebase: You have unstaged changes.\n",
            "error: ",
            "lease commit or stash them.",
        ],
        RebaseWithLocalChanges,
    ),
    (
        &[
            "error: cannot rebase: You have unstaged changes.\n",
            "error: ",
            "lease commit or stash them.",
        ],
        RebaseWithLocalChanges,
    ),
    (
        &["error: commit ", " is a merge but no -m option was given"],
        MergeCommitNoMainlineOption,
    ),
    (
        &["fatal: detected dubious ownership in repository at "],
        UnsafeDirectory,
    ),
    (
        &["fatal: path '", "' exists on disk, but not in '"],
        PathExistsButNotInRef,
    ),
    (&["error: GH001: "], PushWithFileSizeExceedingLimit),
    (&["error: GH002: "], HexBranchNameRejected),
    (
        &["error: GH003: Sorry, force-pushing to ", " is not allowed."],
        ForcePushRejected,
    ),
    (
        &[
            "error: GH005: Sorry, refs longer than ",
            " bytes are not allowed",
        ],
        InvalidRefLength,
    ),
    (
        &[
            "error: GH006: Protected branch update failed for ",
            "\nremote: error: At least one approved review is required",
        ],
        ProtectedBranchRequiresReview,
    ),
    (
        &[
            "error: GH006: Protected branch update failed for ",
            "\nremote: error: Cannot force-push to a protected branch",
        ],
        ProtectedBranchForcePush,
    ),
    (
        &[
            "error: GH006: Protected branch update failed for ",
            "\nremote: error: Cannot delete a protected branch",
        ],
        ProtectedBranchDeleteRejected,
    ),
    (
        &[
            "error: GH006: Protected branch update failed for ",
            ".\nremote: error: Required status check \"",
            "\" is expected",
        ],
        ProtectedBranchRequiredStatus,
    ),
    (
        &["error: GH007: Your push would publish a private email address."],
        PushWithPrivateEmail,
    ),
    (
        &["error: GH013: Repository rule violations found for "],
        PushWithSecretDetected,
    ),
    (&["error: gpg failed to sign the data"], GPGFailedToSignData),
    (
        &[
            "CONFLICT (modify/delete): ",
            " deleted in ",
            " and modified in ",
        ],
        ConflictModifyDeletedInBranch,
    ),
];

/// `true` when every fragment occurs in `text`, each after the previous one.
fn matches_in_order(text: &str, fragments: &[&str]) -> bool {
    let mut rest = text;
    for fragment in fragments {
        match rest.find(fragment) {
            Some(at) => rest = &rest[at + fragment.len()..],
            None => return false,
        }
    }
    true
}

/// dugite `parseError`: the first known error whose pattern matches.
pub fn known_git_error(stderr: &str) -> Option<KnownGitError> {
    PATTERNS
        .iter()
        .find(|(fragments, _)| matches_in_order(stderr, fragments))
        .map(|(_, known)| *known)
}

impl KnownGitError {
    /// GHD `isAuthFailureError`.
    pub fn is_auth_failure(self) -> bool {
        matches!(
            self,
            HTTPSAuthenticationFailed | SSHAuthenticationFailed | SSHPermissionDenied
        )
    }

    /// The operation blocked by uncommitted changes (`localChangesOverwrittenHandler`
    /// handles these three).
    pub fn is_blocked_by_local_changes(self) -> bool {
        matches!(
            self,
            LocalChangesOverwritten | MergeWithLocalChanges | RebaseWithLocalChanges
        )
    }

    /// GHD `getDescriptionForError`: the sentence the error dialog shows
    /// instead of git's output, or `None` when GHD shows the output itself.
    /// `settings_menu` names where accounts are managed ("GitHub Desktop >
    /// Settings." / "File > Options.").
    pub fn description(self, settings_menu: &str) -> Option<String> {
        if self.is_auth_failure() {
            return Some(format!(
                "Authentication failed. Some common reasons include:\n\n\
                 - You are not logged in to your account: see {settings_menu}\n\
                 - You may need to log out and log back in to refresh your token.\n\
                 - You do not have permission to access this repository.\n\
                 - The repository is archived on GitHub. Check the repository settings to \
                 confirm you are still permitted to push commits.\n\
                 - If you use SSH authentication, check that your key is added to the \
                 ssh-agent and associated with your account.\n\
                 - If you use SSH authentication, ensure the host key verification passes \
                 for your repository hosting service.\n\
                 - If you used username / password authentication, you might need to use a \
                 Personal Access Token instead of your account password. Check the \
                 documentation of your repository hosting service."
            ));
        }
        let text = match self {
            SSHKeyAuditUnverified => "The SSH key is unverified.",
            RemoteDisconnection => {
                "The remote disconnected. Check your Internet connection and try again."
            }
            HostDown => "The host is down. Check your Internet connection and try again.",
            RebaseConflicts => {
                "We found some conflicts while trying to rebase. Please resolve the conflicts \
                 before continuing."
            }
            MergeConflicts => {
                "We found some conflicts while trying to merge. Please resolve the conflicts \
                 and commit the changes."
            }
            HTTPSRepositoryNotFound | SSHRepositoryNotFound => {
                "The repository does not seem to exist anymore. You may not have access, or it \
                 may have been deleted or renamed."
            }
            PushNotFastForward => {
                "The repository has been updated since you last pulled. Try pulling before \
                 pushing."
            }
            BranchDeletionFailed => "Could not delete the branch. It was probably already deleted.",
            DefaultBranchDeletionFailed => {
                "The branch is the repository's default branch and cannot be deleted."
            }
            RevertConflicts => "To finish reverting, please merge and commit the changes.",
            EmptyRebasePatch => "There aren’t any changes left to apply.",
            NoMatchingRemoteBranch => {
                "There aren’t any remote branches that match the current branch."
            }
            NoExistingRemoteBranch => "The remote branch does not exist.",
            NothingToCommit => "There are no changes to commit.",
            NoSubmoduleMapping => {
                "A submodule was removed from .gitmodules, but the folder still exists in the \
                 repository. Delete the folder, commit the change, then try again."
            }
            SubmoduleRepositoryDoesNotExist => {
                "A submodule points to a location which does not exist."
            }
            InvalidSubmoduleSHA => "A submodule points to a commit which does not exist.",
            LocalPermissionDenied => "Permission denied.",
            InvalidMerge => "This is not something we can merge.",
            InvalidRebase => "This is not something we can rebase.",
            NonFastForwardMergeIntoEmptyHead => {
                "The merge you attempted is not a fast-forward, so it cannot be performed on \
                 an empty branch."
            }
            PatchDoesNotApply => {
                "The requested changes conflict with one or more files in the repository."
            }
            BranchAlreadyExists => "A branch with that name already exists.",
            BadRevision => "Bad revision.",
            NotAGitRepository => "This is not a git repository.",
            CannotMergeUnrelatedHistories => {
                "Unable to merge unrelated histories in this repository."
            }
            LFSAttributeDoesNotMatch => {
                "Git LFS attribute found in global Git configuration does not match expected \
                 value."
            }
            BranchRenameFailed => "The branch could not be renamed.",
            PathDoesNotExist => "The path does not exist on disk.",
            InvalidObjectName => "The object was not found in the Git repository.",
            OutsideRepository => "This path is not a valid path inside the repository.",
            LockFileAlreadyExists => {
                "A lock file already exists in the repository, which blocks this operation \
                 from completing."
            }
            NoMergeToAbort => "There is no merge in progress, so there is nothing to abort.",
            LocalChangesOverwritten => {
                "Unable to switch branches as there are working directory changes which would \
                 be overwritten. Please commit or stash your changes."
            }
            UnresolvedConflicts => "There are unresolved conflicts in the working directory.",
            TagAlreadyExists => "A tag with that name already exists",
            PushWithFileSizeExceedingLimit => {
                "The push operation includes a file which exceeds GitHub's file size \
                 restriction of 100MB. Please remove the file from history and try again."
            }
            HexBranchNameRejected => {
                "The branch name cannot be a 40-character string of hexadecimal characters, as \
                 this is the format that Git uses for representing objects."
            }
            ForcePushRejected => "The force push has been rejected for the current branch.",
            InvalidRefLength => "A ref cannot be longer than 255 characters.",
            ProtectedBranchRequiresReview => {
                "This branch is protected and any changes requires an approved review. Open a \
                 pull request with changes targeting this branch instead."
            }
            ProtectedBranchForcePush => "This branch is protected from force-push operations.",
            ProtectedBranchDeleteRejected => {
                "This branch cannot be deleted from the remote repository because it is marked \
                 as protected."
            }
            ProtectedBranchRequiredStatus => {
                "The push was rejected by the remote server because a required status check \
                 has not been satisfied."
            }
            PushWithPrivateEmail => {
                "Cannot push these commits as they contain an email address marked as private \
                 on GitHub. To push anyway, visit https://github.com/settings/emails, uncheck \
                 \"Keep my email address private\", then switch back to GitHub Desktop to push \
                 your commits. You can then enable the setting again."
            }
            // GHD shows git's output (or a dedicated dialog) for the rest
            HTTPSAuthenticationFailed
            | SSHAuthenticationFailed
            | SSHPermissionDenied
            | ConfigLockFileAlreadyExists
            | RemoteAlreadyExists
            | MergeWithLocalChanges
            | RebaseWithLocalChanges
            | MergeCommitNoMainlineOption
            | UnsafeDirectory
            | PathExistsButNotInRef
            | PushWithSecretDetected
            | GPGFailedToSignData
            | ConflictModifyDeletedInBranch => return None,
        };
        Some(text.to_string())
    }

    /// Corvene's lead sentence for the errors GHD leaves undescribed; the
    /// GHD description otherwise.
    pub fn lead(self, settings_menu: &str) -> Option<String> {
        let text = match self {
            MergeWithLocalChanges => {
                "Unable to merge as there are working directory changes which would be \
                 overwritten."
            }
            RebaseWithLocalChanges => {
                "Unable to rebase as there are uncommitted changes in the working directory."
            }
            ConfigLockFileAlreadyExists => {
                "Another program is changing this repository's Git configuration; try again in \
                 a moment."
            }
            RemoteAlreadyExists => "A remote with that name already exists.",
            MergeCommitNoMainlineOption => {
                "This is a merge commit; Git needs to know which parent to compare it with."
            }
            PathExistsButNotInRef => "The file exists on disk but not in that commit.",
            GPGFailedToSignData => "Failed to sign data.",
            ConflictModifyDeletedInBranch => {
                "A file was deleted on one side and modified on the other, which Git cannot \
                 merge on its own."
            }
            _ => return self.description(settings_menu),
        };
        Some(text.to_string())
    }
}

/// Corvene (`1104-lfs-server-authentication`): git-lfs could not sign in to
/// its server ("batch response: too many authentication attempts" when the
/// credential prompt was answered with nothing, "Git credentials for … not
/// found.", "Authorization error: …").
pub fn is_lfs_auth_failure(stderr: &str) -> bool {
    stderr.lines().any(|line| {
        line.starts_with("batch response:")
            && [
                "too many authentication attempts",
                "Git credentials for ",
                "Authorization error",
                "Authentication required",
                "Bad credentials",
            ]
            .iter()
            .any(|marker| line.contains(marker))
    })
}

/// The Git LFS server of an [`is_lfs_auth_failure`], when git-lfs named it:
/// "Git credentials for <url> not found.", "Authorization error: <url>", or
/// the "could not read Username for '<url>'" of the prompt it asked for.
pub fn lfs_auth_failure_url(stderr: &str) -> Option<String> {
    let url_after = |marker: &str| {
        stderr.lines().find_map(|line| {
            let rest = &line[line.find(marker)? + marker.len()..];
            let url = rest.split_whitespace().next()?;
            let url = url.trim_end_matches(['.', ',', ':', '\'', '"', ')']);
            (url.starts_with("http://") || url.starts_with("https://")).then(|| url.to_string())
        })
    };
    url_after("Git credentials for ")
        .or_else(|| {
            stderr
                .contains("batch response")
                .then(|| url_after("Authorization error: "))
                .flatten()
        })
        .or_else(|| {
            (stderr.contains("LFS") || stderr.contains("git-lfs"))
                .then(|| url_after("could not read Username for '"))
                .flatten()
        })
}

/// GHD `parseFilesToBeOverwritten` (`ui/lib/parse-files-to-be-overwritten.ts`):
/// the tab-indented files git lists under the first `error: … files would be
/// overwritten …:` line ("Your local changes to the following files would be
/// overwritten by checkout:", "The following untracked working tree files
/// would be overwritten by merge:", …), up to the first line that is not
/// indented.
pub fn files_that_would_be_overwritten(stderr: &str) -> Vec<String> {
    let mut files = Vec::new();
    let mut in_list = false;
    for line in stderr.split('\n') {
        if in_list {
            match line.strip_prefix('\t') {
                Some(_) => files.push(line.trim_start().to_string()),
                None => break,
            }
        } else if line.starts_with("error:")
            && line.contains("files would be overwritten")
            && line.ends_with(':')
        {
            in_list = true;
        }
    }
    files
}

/// Corvene (`1206-explain-merge-abort-failure`): the files of `git merge
/// --abort`'s "error: Entry '<path>' not uptodate. Cannot merge." lines,
/// files that changed after the merge started, which keep git from
/// resetting the index.
pub fn merge_abort_blocked_paths(stderr: &str) -> Vec<String> {
    stderr
        .lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("error: Entry '")?;
            let path = rest.strip_suffix("' not uptodate. Cannot merge.")?;
            (!path.is_empty()).then(|| path.to_string())
        })
        .collect()
}

/// GHD `parseConfigLockFilePathFromError` (`lib/git/core.ts`): the lock file
/// of git's "error: could not lock config file <path>: File exists"
/// (`<path>.lock`), resolved against `path`, the directory git ran in, as
/// `Path.resolve` does (`.` and `..` folded). Windows: the first `/` of the
/// path becomes `\`, as in GHD; the rest are normalised by the resolving.
pub fn parse_config_lock_file_path_from_error(stderr: &str, path: &Path) -> Option<PathBuf> {
    const PREFIX: &str = "error: could not lock config file ";
    const SUFFIX: &str = ": File exists";
    // `/^error: could not lock config file (.+?): File exists$/m`
    let config = stderr
        .split(['\n', '\r', '\u{2028}', '\u{2029}'])
        .find_map(|line| {
            let rest = line.strip_prefix(PREFIX)?.strip_suffix(SUFFIX)?;
            (!rest.is_empty()).then_some(rest)
        })?;
    #[cfg(windows)]
    let config = config.replacen('/', "\\", 1);
    Some(crate::ops::resolve(&path.join(format!("{config}.lock"))))
}

/// What git named in its output, pulled out so the error dialog can show it
/// as lists and labelled values instead of a wall of text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GitErrorDetails {
    /// Files git listed, with what the list means ("Files that would be
    /// overwritten", "Conflicted files", …).
    pub files: Option<(&'static str, Vec<String>)>,
    /// Refs a push rejected: `local -> remote` and git's reason.
    pub rejected: Vec<(String, String)>,
    /// What the server said (`remote:` lines, unprefixed, blank lines and
    /// GitHub's rules dropped).
    pub remote: Vec<String>,
    /// git's `hint:` lines, unprefixed.
    pub hints: Vec<String>,
    /// A path or URL git singled out, labelled ("Worktree", "Lock file",
    /// "Remote", "Path", "Submodule", "Repository").
    pub subject: Option<(&'static str, String)>,
}

impl GitErrorDetails {
    pub fn is_empty(&self) -> bool {
        self.files.is_none()
            && self.rejected.is_empty()
            && self.remote.is_empty()
            && self.hints.is_empty()
            && self.subject.is_none()
    }
}

/// The text between the first pair of single quotes after `marker`.
fn quoted_after<'a>(line: &'a str, marker: &str) -> Option<&'a str> {
    let rest = &line[line.find(marker)? + marker.len()..];
    let rest = rest.strip_prefix('\'').or_else(|| {
        let at = rest.find('\'')?;
        Some(&rest[at + 1..])
    })?;
    let end = rest.find('\'')?;
    Some(&rest[..end])
}

/// Parse git's output into [`GitErrorDetails`].
pub fn git_error_details(output: &str) -> GitErrorDetails {
    let mut d = GitErrorDetails::default();
    let overwritten = files_that_would_be_overwritten(output);
    if !overwritten.is_empty() {
        d.files = Some(("Files that would be overwritten", overwritten));
    }
    let mut conflicted = Vec::new();
    let mut unapplied = Vec::new();
    for raw in output.lines() {
        let line = raw.trim_end();
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("remote: ") {
            let rest = rest.trim();
            if !rest.is_empty() && !rest.chars().all(|c| c == '-' || c == '=') {
                d.remote.push(rest.to_string());
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("hint: ") {
            d.hints.push(rest.trim().to_string());
            continue;
        }
        if trimmed == "hint:" {
            continue;
        }
        if let Some(rest) = trimmed
            .strip_prefix("! [rejected] ")
            .or_else(|| trimmed.strip_prefix("! [remote rejected] "))
        {
            let rest = rest.trim();
            let (refs, reason) = match rest.rfind(" (") {
                Some(at) if rest.ends_with(')') => (&rest[..at], &rest[at + 2..rest.len() - 1]),
                _ => (rest, ""),
            };
            d.rejected
                .push((refs.trim().to_string(), reason.to_string()));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("CONFLICT (") {
            // `CONFLICT (content): Merge conflict in <path>`,
            // `CONFLICT (modify/delete): <path> deleted in …`
            if let Some(path) = rest.split_once("Merge conflict in ").map(|(_, p)| p) {
                conflicted.push(path.to_string());
            } else if let Some((_, after)) = rest.split_once("): ")
                && let Some((path, _)) = after.split_once(" deleted in ")
            {
                conflicted.push(path.to_string());
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("error: ")
            && let Some((path, _)) = rest
                .split_once(": patch does not apply")
                .or_else(|| rest.split_once(": already exists in working directory"))
        {
            unapplied.push(path.to_string());
            continue;
        }
        if d.subject.is_none() {
            d.subject = if trimmed.contains("used by worktree at '")
                || trimmed.contains("checked out at '")
            {
                crate::error::branch_in_other_worktree(trimmed)
                    .map(|(_, path)| ("Worktree", path.display().to_string()))
            } else if trimmed.contains("index.lock") {
                crate::index_lock::index_lock_path(trimmed)
                    .map(|p| ("Lock file", p.display().to_string()))
            } else if let Some(url) = quoted_after(trimmed, "unable to access ") {
                Some(("Remote", url.to_string()))
            } else if let Some(url) = quoted_after(trimmed, "fatal: repository ") {
                Some(("Repository", url.to_string()))
            } else if let Some(path) = quoted_after(trimmed, "submodule path ") {
                Some(("Submodule", path.to_string()))
            } else if let Some(path) = quoted_after(trimmed, "fatal: path ") {
                Some(("Path", path.to_string()))
            } else if let Some(path) = quoted_after(trimmed, "could not create work tree dir ") {
                Some(("Folder", path.to_string()))
            } else {
                None
            };
        }
    }
    if d.files.is_none() && !conflicted.is_empty() {
        d.files = Some(("Conflicted files", conflicted));
    }
    if d.files.is_none() && !unapplied.is_empty() {
        d.files = Some(("Files the changes do not apply to", unapplied));
    }
    d
}

/// A failed git command as the error dialog shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitFailure {
    /// The command as run (`git` plus its arguments), without the `-c
    /// key=value` configuration Corvene prepends.
    pub command: String,
    pub exit_code: Option<i32>,
    /// git's output as GHD's `GitError` message has it: stdout and stderr
    /// (the last 256 KiB), untrimmed.
    pub output: String,
    pub known: Option<KnownGitError>,
}

impl GitFailure {
    pub fn new(args: &str, exit_code: Option<i32>, output: &str) -> Self {
        Self {
            command: display_command(args),
            exit_code,
            output: output.to_string(),
            known: known_git_error(output),
        }
    }

    /// `git <command> failed with exit code N:` and the output, as one text.
    pub fn summary(&self) -> String {
        let code = match self.exit_code {
            Some(code) => code.to_string(),
            None => "none (killed by a signal)".to_string(),
        };
        format!(
            "{} failed with exit code {code}:\n{}",
            self.command,
            self.output.trim_end()
        )
    }

    /// [`git_error_details`] of the output.
    pub fn details(&self) -> GitErrorDetails {
        git_error_details(&self.output)
    }

    /// GHD's description for this failure, when it has one.
    pub fn description(&self, settings_menu: &str) -> Option<String> {
        self.known.and_then(|k| k.description(settings_menu))
    }

    /// The lead sentence of the structured dialog: GHD's description or
    /// Corvene's for a recognised error, Corvene's for a branch another
    /// worktree has checked out, else git's first error line.
    pub fn lead(&self, settings_menu: &str) -> Option<String> {
        if let Some(lead) = self.known.and_then(|k| k.lead(settings_menu)) {
            return Some(lead);
        }
        if let Some((branch, path)) = crate::error::branch_in_other_worktree(&self.output) {
            return Some(format!(
                "\"{branch}\" is checked out in the worktree at {}. Switch that worktree to \
                 another branch, or remove it, to use \"{branch}\" here.",
                path.display()
            ));
        }
        self.first_error_line()
    }

    /// The first `fatal:` / `error:` line of the output as a sentence, for
    /// failures nobody has words for: skipped when it introduces a list.
    pub fn first_error_line(&self) -> Option<String> {
        self.output.lines().find_map(|line| {
            let line = line.trim();
            let rest = line
                .strip_prefix("fatal: ")
                .or_else(|| line.strip_prefix("error: "))?;
            let rest = rest.trim();
            if rest.is_empty() || rest.ends_with(':') || rest.chars().count() > 160 {
                return None;
            }
            let mut chars = rest.chars();
            let first = chars.next()?.to_uppercase().collect::<String>();
            let mut sentence = first + chars.as_str();
            if !sentence.ends_with(['.', '!', '?']) {
                sentence.push('.');
            }
            Some(sentence)
        })
    }
}

/// `git <args>` for display: leading `-c key=value` pairs (Corvene's own
/// configuration, never typed by the user) are left out.
pub fn display_command(args: &str) -> String {
    let mut tokens = args.split_whitespace().peekable();
    while tokens.peek() == Some(&"-c") {
        tokens.next();
        tokens.next();
    }
    let rest = tokens.collect::<Vec<_>>().join(" ");
    if rest.is_empty() {
        "git".to_string()
    } else {
        format!("git {rest}")
    }
}

impl GitError {
    /// The failed command behind this error, when git ran and exited non-zero.
    pub fn failure(&self) -> Option<GitFailure> {
        match self {
            GitError::Failed { args, code, stderr } => Some(GitFailure::new(args, *code, stderr)),
            _ => None,
        }
    }

    /// [`known_git_error`] of a failed command.
    pub fn known(&self) -> Option<KnownGitError> {
        match self {
            GitError::Failed { stderr, .. } => known_git_error(stderr),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn lfs_sign_in_failures_name_the_lfs_server() {
        // git-lfs 3.8 pushing to a server that wants a login
        let push = "error: unable to read askpass response from '/x/corvene'\n\
                    fatal: could not read Username for 'http://127.0.0.1:18767': terminal prompts disabled\n\
                    batch response: Git credentials for http://127.0.0.1:18767/repo.git/info/lfs not found.\n\
                    Uploading LFS objects:   0% (0/1), 0 B | 0 B/s, done.\n\
                    error: failed to push some refs to 'https://github.com/o/r.git'\n";
        assert_eq!(
            lfs_auth_failure_url(push).as_deref(),
            Some("http://127.0.0.1:18767/repo.git/info/lfs")
        );
        let refused = "batch response: Authorization error: https://lfs.corp/o/r/info/lfs/objects/batch\n\
                       Check that you have proper access to the repository\n";
        assert_eq!(
            lfs_auth_failure_url(refused).as_deref(),
            Some("https://lfs.corp/o/r/info/lfs/objects/batch")
        );
        // plain git's own sign-in failure is not one
        assert_eq!(
            lfs_auth_failure_url(
                "fatal: could not read Username for 'https://github.com': terminal prompts disabled\n"
            ),
            None
        );
        // Corvene's askpass answers an unknown host with nothing
        let empty = "Uploading LFS objects:   0% (0/1), 0 B | 0 B/s, done.\n\
                     batch response: too many authentication attempts\n\
                     error: failed to push some refs to '/srv/remote.git'\n";
        assert!(is_lfs_auth_failure(empty) && is_lfs_auth_failure(push));
        assert_eq!(lfs_auth_failure_url(empty), None);
        assert!(!is_lfs_auth_failure(
            "remote: Invalid username or password.\nfatal: Authentication failed for 'https://github.com/o/r.git/'\n"
        ));
    }

    use super::*;

    const PULL_OVERWRITTEN: &str = "Updating e9455681..1f7e3d4e\n\
        error: Your local changes to the following files would be overwritten by merge:\n\
        \tcrates/corvene-platform/src/editors.rs\n\
        \tcrates/corvene-ui/src/widgets.rs\n\
        Please commit your changes or stash them before you merge.\n\
        Aborting";

    #[test]
    fn merge_abort_names_the_files_that_changed() {
        let stderr = "error: Entry 'b.txt' not uptodate. Cannot merge.\n\
                      error: Entry 'dir/c d.txt' not uptodate. Cannot merge.\n\
                      fatal: Could not reset index file to revision 'HEAD'.\n";
        assert_eq!(merge_abort_blocked_paths(stderr), ["b.txt", "dir/c d.txt"]);
        assert!(merge_abort_blocked_paths("fatal: There is no merge to abort").is_empty());
    }

    #[test]
    fn recognises_dugite_errors() {
        let cases: &[(&str, KnownGitError)] = &[
            (PULL_OVERWRITTEN, MergeWithLocalChanges),
            (
                "error: Your local changes to the following files would be overwritten by checkout:\n\tREADME.md\nPlease commit your changes or stash them before you switch branches.\nAborting",
                LocalChangesOverwritten,
            ),
            (
                "error: cannot pull with rebase: You have unstaged changes.\nerror: please commit or stash them.",
                RebaseWithLocalChanges,
            ),
            (
                "To github.com:o/r.git\n ! [rejected]        main -> main (fetch first)\nerror: failed to push some refs to 'github.com:o/r.git'",
                PushNotFastForward,
            ),
            (
                "fatal: Authentication failed for 'https://github.com/o/r.git/'",
                HTTPSAuthenticationFailed,
            ),
            (
                "remote: error: GH006: Protected branch update failed for refs/heads/main.\nremote: error: At least one approved review is required by reviewers with write access.",
                ProtectedBranchRequiresReview,
            ),
            (
                "fatal: A branch named 'feature' already exists.",
                BranchAlreadyExists,
            ),
            (
                "fatal: refusing to merge unrelated histories",
                CannotMergeUnrelatedHistories,
            ),
            (
                "fatal: Unable to create '/r/.git/index.lock': File exists.\n\nAnother git process seems to be running in this repository, e.g.\nan editor opened by 'git commit'.",
                LockFileAlreadyExists,
            ),
            (
                "Auto-merging a.txt\nCONFLICT (content): Merge conflict in a.txt\nAutomatic merge failed; fix conflicts and then commit the result.",
                MergeConflicts,
            ),
        ];
        for (stderr, expected) in cases {
            assert_eq!(known_git_error(stderr), Some(*expected), "{stderr}");
        }
        assert_eq!(known_git_error("fatal: something new"), None);
    }

    #[test]
    fn descriptions_follow_ghd() {
        assert_eq!(
            PushNotFastForward.description("x").as_deref(),
            Some(
                "The repository has been updated since you last pulled. Try pulling before pushing."
            )
        );
        assert!(
            HTTPSAuthenticationFailed
                .description("File > Options.")
                .is_some_and(|d| d.contains("File > Options."))
        );
        assert_eq!(MergeWithLocalChanges.description("x"), None);
        assert!(MergeWithLocalChanges.lead("x").is_some());
    }

    #[test]
    fn lists_overwritten_files() {
        assert_eq!(
            files_that_would_be_overwritten(PULL_OVERWRITTEN),
            vec![
                "crates/corvene-platform/src/editors.rs",
                "crates/corvene-ui/src/widgets.rs"
            ]
        );
        assert!(files_that_would_be_overwritten("fatal: nope").is_empty());
        // any `error: … files would be overwritten …:` header; the first
        // list only; leading white space dropped, trailing kept
        let stderr = "error: The following untracked working tree files would be overwritten by merge:\n\
                      \ta.txt \n\
                      Please move or remove them before you merge.\n\
                      error: Your local changes to the following files would be overwritten by checkout:\n\
                      \tb.txt\n";
        assert_eq!(files_that_would_be_overwritten(stderr), vec!["a.txt "]);
    }

    #[test]
    fn parses_the_config_lock_file_path() {
        let stderr = "error: could not lock config file .git/config: File exists\n";
        // Windows: `/r` is not absolute
        if cfg!(not(windows)) {
            assert_eq!(
                parse_config_lock_file_path_from_error(stderr, Path::new("/r/sub/..")),
                Some(PathBuf::from("/r/.git/config.lock"))
            );
        }
        assert_eq!(
            parse_config_lock_file_path_from_error(
                "error: could not lock config file : File exists",
                Path::new("/")
            ),
            None
        );
        assert_eq!(
            parse_config_lock_file_path_from_error("fatal: nope", Path::new("/")),
            None
        );
    }

    #[test]
    fn parses_details() {
        let d = git_error_details(PULL_OVERWRITTEN);
        assert_eq!(
            d.files,
            Some((
                "Files that would be overwritten",
                vec![
                    "crates/corvene-platform/src/editors.rs".to_string(),
                    "crates/corvene-ui/src/widgets.rs".to_string()
                ]
            ))
        );
        assert!(d.rejected.is_empty() && d.remote.is_empty() && d.subject.is_none());

        let push = "remote: error: GH006: Protected branch update failed for refs/heads/main.\n\
                    remote: \n\
                    remote: - Changes must be made through a pull request.\n\
                    To github.com:o/r.git\n \
                    ! [remote rejected] main -> main (protected branch hook declined)\n\
                    error: failed to push some refs to 'github.com:o/r.git'";
        let d = git_error_details(push);
        assert_eq!(
            d.remote,
            vec![
                "error: GH006: Protected branch update failed for refs/heads/main.",
                "- Changes must be made through a pull request."
            ]
        );
        assert_eq!(
            d.rejected,
            vec![(
                "main -> main".to_string(),
                "protected branch hook declined".to_string()
            )]
        );

        let merge = "Auto-merging a.txt\nCONFLICT (content): Merge conflict in a.txt\n\
                     CONFLICT (modify/delete): b.txt deleted in HEAD and modified in topic.\n\
                     Automatic merge failed; fix conflicts and then commit the result.";
        assert_eq!(
            git_error_details(merge).files,
            Some((
                "Conflicted files",
                vec!["a.txt".to_string(), "b.txt".to_string()]
            ))
        );

        let d = git_error_details("fatal: 'main' is already used by worktree at '/Users/o/w 2'");
        assert_eq!(d.subject, Some(("Worktree", "/Users/o/w 2".to_string())));
        let d = git_error_details(
            "fatal: unable to access 'https://github.com/o/r.git/': Could not resolve host: github.com",
        );
        assert_eq!(
            d.subject,
            Some(("Remote", "https://github.com/o/r.git/".to_string()))
        );
        let d = git_error_details(
            "fatal: Unable to create '/r/.git/index.lock': File exists.\n\nAnother git process seems to be running in this repository, e.g.\nan editor opened by 'git commit'.",
        );
        assert_eq!(
            d.subject,
            Some(("Lock file", "/r/.git/index.lock".to_string()))
        );
        let d = git_error_details(
            "error: could not apply 1234abc... x\nhint: after resolving the conflicts, mark the corrected paths\nhint: with 'git add <paths>' or 'git rm <paths>'",
        );
        assert_eq!(d.hints.len(), 2);
        assert!(git_error_details("fatal: nothing").is_empty());
    }

    #[test]
    fn failure_strips_config_and_finds_lead() {
        let failure = GitFailure::new(
            "-c rebase.backend=merge pull --ff --recurse-submodules --progress origin",
            Some(1),
            PULL_OVERWRITTEN,
        );
        assert_eq!(
            failure.command,
            "git pull --ff --recurse-submodules --progress origin"
        );
        assert_eq!(failure.known, Some(MergeWithLocalChanges));
        // the first error line introduces a list: no sentence from it
        assert_eq!(failure.first_error_line(), None);
        let worktree = GitFailure::new(
            "checkout main --",
            Some(128),
            "fatal: 'main' is already used by worktree at '/Users/o/corvene'",
        );
        assert_eq!(worktree.known, None);
        assert!(
            worktree
                .lead("x")
                .is_some_and(|l| l
                    .starts_with("\"main\" is checked out in the worktree at /Users/o/corvene.")),
            "{:?}",
            worktree.lead("x")
        );
        let other = GitFailure::new("checkout x", Some(128), "fatal: invalid reference: x");
        assert_eq!(
            other.first_error_line().as_deref(),
            Some("Invalid reference: x.")
        );
        assert_eq!(display_command("-c a=b"), "git");
    }
}
