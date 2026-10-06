//! `.gitignore` edits - GHD `lib/git/gitignore.ts` (`appendIgnoreRule`,
//! `appendIgnoreFile`, `escapeGitSpecialCharacters`) - and [`IgnoreMatcher`],
//! gitoxide's exclude stack for the filesystem watcher (a Corvene addition).
//!
//! Deviation (flag `ignore-skips-existing-rules`): patterns already in the
//! file are not appended again.
//!
//! As in GHD (`openExistingGitIgnore`), the root `.gitignore` is never read
//! or written through a symbolic link; Corvene refuses one in a directory's
//! `.gitignore` (flag `ignore-file-targets`) too, while `info/exclude` and
//! the global excludes file may be links.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use gix::bstr::ByteSlice;

use crate::detect::GitBinary;
use crate::error::{GitError, Result};
use crate::process::GitCommand;

/// Where an "Ignore File" menu item writes (Corvene, flag
/// `ignore-file-targets`; GHD always uses the root `.gitignore`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IgnoreTarget {
    /// `<workdir>/.gitignore` (GHD).
    Root,
    /// The `.gitignore` of a repository-relative directory (no trailing `/`).
    Directory(String),
    /// `$GIT_DIR/info/exclude`: this clone only, never committed.
    InfoExclude,
    /// `core.excludesFile` (default `$XDG_CONFIG_HOME/git/ignore`): every
    /// repository on this computer.
    ExcludesFile,
}

impl IgnoreTarget {
    /// The pattern that ignores the repository-relative `path` from this
    /// target: the root and `info/exclude` take the path, a directory's
    /// `.gitignore` the anchored rest below it, the global file the file name.
    pub fn pattern_for(&self, path: &str) -> String {
        match self {
            Self::Root | Self::InfoExclude => escape_gitignore_pattern(path),
            Self::Directory(dir) => {
                let rest = path
                    .strip_prefix(dir.as_str())
                    .and_then(|r| r.strip_prefix('/'))
                    .unwrap_or(path);
                format!("/{}", escape_gitignore_pattern(rest))
            }
            Self::ExcludesFile => escape_gitignore_pattern(path.rsplit('/').next().unwrap_or(path)),
        }
    }
}

/// Directories above `path` (repository-relative) that have a `.gitignore`,
/// nearest first; the root is left out.
pub fn gitignore_dirs_above(workdir: &Path, path: &str) -> Vec<String> {
    let mut dirs = Vec::new();
    let mut dir = path;
    while let Some((parent, _)) = dir.rsplit_once('/') {
        if workdir.join(parent).join(".gitignore").is_file() {
            dirs.push(parent.to_string());
        }
        dir = parent;
    }
    dirs
}

/// The global excludes file: `core.excludesFile` as git sees it from
/// `workdir` (so a repository's own setting wins), else
/// `$XDG_CONFIG_HOME/git/ignore` / `~/.config/git/ignore`.
pub fn excludes_file(git: Arc<GitBinary>, workdir: &Path) -> Option<PathBuf> {
    let configured = GitCommand::new(git)
        .args(["config", "--type=path", "--get", "core.excludesFile"])
        .current_dir(workdir)
        .allow_exit_code(1)
        .run()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| o.stdout_string().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Some(path) = configured {
        let path = PathBuf::from(path);
        return Some(if path.is_absolute() {
            path
        } else {
            workdir.join(path)
        });
    }
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(|home| PathBuf::from(home).join(".config"))
        })?;
    Some(config_home.join("git").join("ignore"))
}

/// The file behind an [`IgnoreTarget`].
pub fn ignore_target_path(
    git: Arc<GitBinary>,
    workdir: &Path,
    target: &IgnoreTarget,
) -> Result<PathBuf> {
    Ok(match target {
        IgnoreTarget::Root => workdir.join(".gitignore"),
        IgnoreTarget::Directory(dir) => workdir.join(dir).join(".gitignore"),
        IgnoreTarget::InfoExclude => {
            let out = GitCommand::new(git)
                .args(["rev-parse", "--git-path", "info/exclude"])
                .current_dir(workdir)
                .run()?;
            let path = PathBuf::from(out.stdout_string()?.trim());
            if path.is_absolute() {
                path
            } else {
                workdir.join(path)
            }
        }
        IgnoreTarget::ExcludesFile => excludes_file(git, workdir).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No global ignore file: core.excludesFile is unset and HOME is unknown",
            )
        })?,
    })
}

/// Append patterns to the file behind `target`.
pub fn append_ignore_rules_to(
    git: Arc<GitBinary>,
    workdir: &Path,
    target: &IgnoreTarget,
    patterns: &[String],
    skip_existing: bool,
) -> Result<()> {
    let path = ignore_target_path(git, workdir, target)?;
    // the repository's own files must not lead outside it (GHD checks the
    // root file); `info/exclude` and the global file are the user's
    let no_follow = match target {
        IgnoreTarget::Root => Some(SYMBOLIC_LINK_ERROR),
        IgnoreTarget::Directory(_) => Some(NESTED_SYMBOLIC_LINK_ERROR),
        IgnoreTarget::InfoExclude | IgnoreTarget::ExcludesFile => None,
    };
    append_to_ignore_file(&path, patterns, skip_existing, no_follow)
}

/// Escape the characters git treats specially in a pattern: `[ ] ! * # ?`.
pub fn escape_gitignore_pattern(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for c in path.chars() {
        if matches!(c, '[' | ']' | '!' | '*' | '#' | '?') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Append raw patterns to the root `.gitignore`, creating it if needed. Keeps
/// the file's existing line endings (GHD consults `core.autocrlf`). With
/// `skip_existing`, patterns already in the file as a line (or earlier in
/// `patterns`) are not added again; GHD appends them blindly. A symbolic link
/// in its place is refused, as [`read_gitignore`] and [`save_gitignore`] do.
pub fn append_ignore_rules(workdir: &Path, patterns: &[String], skip_existing: bool) -> Result<()> {
    append_to_ignore_file(
        &workdir.join(".gitignore"),
        patterns,
        skip_existing,
        Some(SYMBOLIC_LINK_ERROR),
    )
}

/// [`append_ignore_rules`] for any ignore file; missing parent directories
/// are created. With `no_follow` (the error to give) a symbolic link at
/// `path` is refused instead of written through.
fn append_to_ignore_file(
    path: &Path,
    patterns: &[String],
    skip_existing: bool,
    no_follow: Option<&str>,
) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let existing = match no_follow {
        Some(message) => read_no_follow(path, message)?,
        None => match std::fs::read_to_string(path) {
            Ok(text) => Some(text),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
            Err(err) => return Err(err.into()),
        },
    };
    let mut text = existing.unwrap_or_default();
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    if !text.is_empty() && !text.ends_with('\n') {
        text.push_str(eol);
    }
    let mut existing: std::collections::HashSet<String> = if skip_existing {
        text.lines().map(|line| line.trim().to_string()).collect()
    } else {
        Default::default()
    };
    let before = text.len();
    for pattern in patterns {
        if skip_existing && !existing.insert(pattern.trim().to_string()) {
            continue;
        }
        text.push_str(pattern);
        text.push_str(eol);
    }
    if skip_existing && text.len() == before {
        return Ok(());
    }
    match no_follow {
        Some(message) => write_no_follow(path, &text, message),
        None => Ok(std::fs::write(path, text)?),
    }
}

/// GHD `symbolicLinkErrorMessage`.
const SYMBOLIC_LINK_ERROR: &str = "Cannot use a symbolic link as the root .gitignore file";
/// [`SYMBOLIC_LINK_ERROR`] for a directory's `.gitignore` (Corvene, flag
/// `ignore-file-targets`).
const NESTED_SYMBOLIC_LINK_ERROR: &str = "Cannot use a symbolic link as a .gitignore file";

fn symbolic_link_error(message: &str) -> GitError {
    GitError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        message.to_string(),
    ))
}

/// GHD `ensureGitIgnoreIsNotSymbolicLink`: fails when `path` is a symbolic
/// link (live or dangling); a missing path is fine.
fn ensure_not_symbolic_link(path: &Path, message: &str) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Err(symbolic_link_error(message)),
        Ok(_) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

/// GHD `openExistingGitIgnore`: `path` opened without following a symbolic
/// link (`O_NOFOLLOW`; on Windows the link itself is opened), `None` when it
/// does not exist. A symbolic link, or a file that is not the one at `path`
/// any more (replaced between the open and the check), is refused.
fn open_existing_no_follow(
    path: &Path,
    options: &mut std::fs::OpenOptions,
    message: &str,
) -> Result<Option<std::fs::File>> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_FLAG_OPEN_REPARSE_POINT
        options.custom_flags(0x0020_0000);
    }
    let file = match options.open(path) {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            ensure_not_symbolic_link(path, message)?;
            return Ok(None);
        }
        #[cfg(unix)]
        Err(err) if err.raw_os_error() == Some(libc::ELOOP) => {
            return Err(symbolic_link_error(message));
        }
        Err(err) => return Err(err.into()),
    };
    let path_meta = std::fs::symlink_metadata(path)?;
    if path_meta.file_type().is_symlink() {
        return Err(symbolic_link_error(message));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let file_meta = file.metadata()?;
        if file_meta.dev() != path_meta.dev() || file_meta.ino() != path_meta.ino() {
            return Err(symbolic_link_error(message));
        }
    }
    Ok(Some(file))
}

/// [`open_existing_no_follow`] for reading, then the text.
fn read_no_follow(path: &Path, message: &str) -> Result<Option<String>> {
    use std::io::Read;
    let Some(mut file) =
        open_existing_no_follow(path, std::fs::OpenOptions::new().read(true), message)?
    else {
        return Ok(None);
    };
    let mut text = String::new();
    file.read_to_string(&mut text)?;
    Ok(Some(text))
}

/// GHD `saveGitIgnore`'s write: the existing file (opened without following
/// a link) truncated and rewritten, or a new one created exclusively, so a
/// symbolic link put there meanwhile is never followed.
fn write_no_follow(path: &Path, text: &str, message: &str) -> Result<()> {
    use std::io::Write;
    let mut file =
        match open_existing_no_follow(path, std::fs::OpenOptions::new().write(true), message)? {
            Some(file) => file,
            None => std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)?,
        };
    file.set_len(0)?;
    file.write_all(text.as_bytes())?;
    Ok(())
}

/// The root `.gitignore` text, `None` when the file does not exist
/// (GHD `readGitIgnoreAtRoot`). A symbolic link there, live or dangling, is
/// refused ("Cannot use a symbolic link as the root .gitignore file").
pub fn read_gitignore(workdir: &Path) -> Result<Option<String>> {
    read_no_follow(&workdir.join(".gitignore"), SYMBOLIC_LINK_ERROR)
}

/// GHD `saveGitIgnore`: empty text deletes the file; otherwise the text is
/// written with a trailing newline, using CRLF when `core.autocrlf` is on.
/// A symbolic link in the file's place is refused and its target left alone.
pub fn save_gitignore(workdir: &Path, text: &str, autocrlf: bool) -> Result<()> {
    let path = workdir.join(".gitignore");
    if text.is_empty() {
        ensure_not_symbolic_link(&path, SYMBOLIC_LINK_ERROR)?;
        return match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err.into()),
        };
    }
    let eol = if autocrlf { "\r\n" } else { "\n" };
    let mut out = String::with_capacity(text.len() + 2);
    for line in text.replace("\r\n", "\n").split('\n') {
        out.push_str(line);
        out.push_str(eol);
    }
    // `split` yields a trailing empty piece when the text ends with a newline.
    while out.ends_with(&format!("{eol}{eol}")) {
        out.truncate(out.len() - eol.len());
    }
    write_no_follow(&path, &out, SYMBOLIC_LINK_ERROR)
}

/// Ignore file paths (escaped first), as the "Ignore File" menu items do.
pub fn append_ignore_files(workdir: &Path, paths: &[String], skip_existing: bool) -> Result<()> {
    let patterns: Vec<String> = paths.iter().map(|p| escape_gitignore_pattern(p)).collect();
    append_ignore_rules(workdir, &patterns, skip_existing)
}

/// Corvene `1308-ignore-oversized-files`: the ones of the
/// repository-relative `paths` the index tracks (`git ls-files`), in order.
pub fn tracked_paths(git: Arc<GitBinary>, workdir: &Path, paths: &[String]) -> Result<Vec<String>> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let text = GitCommand::new(git)
        .args(["ls-files", "-z", "--"])
        .args(paths)
        .env("GIT_LITERAL_PATHSPECS", "1")
        .current_dir(workdir)
        .run()?
        .stdout_string()?;
    let tracked: std::collections::HashSet<&str> =
        text.split('\0').filter(|p| !p.is_empty()).collect();
    Ok(paths
        .iter()
        .filter(|p| tracked.contains(p.as_str()))
        .cloned()
        .collect())
}

/// Corvene `1308-ignore-oversized-files`: ignore `paths` in the root
/// `.gitignore` ([`append_ignore_files`]) and, with `untrack`, remove the
/// tracked ones from the index (`git rm --cached`), so a file the last
/// commit has becomes a deletion and the copy on disk stays.
pub fn ignore_and_untrack(
    git: Arc<GitBinary>,
    workdir: &Path,
    paths: &[String],
    untrack: bool,
    skip_existing: bool,
) -> Result<()> {
    append_ignore_files(workdir, paths, skip_existing)?;
    if untrack && !paths.is_empty() {
        GitCommand::new(git)
            .args(["rm", "--cached", "-q", "--ignore-unmatch", "--"])
            .args(paths)
            .env("GIT_LITERAL_PATHSPECS", "1")
            .current_dir(workdir)
            .run()?;
    }
    Ok(())
}

/// Answers "would git ignore this worktree path?" the way `git status` does:
/// the `.gitignore` files on the way to the path (read lazily as paths are
/// asked about), `$GIT_DIR/info/exclude` and `core.excludesFile` (or the XDG
/// default), with an ignored directory ignoring everything below it. Paths
/// the index tracks never count as ignored, since git still reports their
/// changes. The index and the global files are read once: build a new matcher
/// after a `.gitignore`, `info/exclude`, the config or the index changes.
pub struct IgnoreMatcher {
    repo: gix::Repository,
    index: gix::worktree::Index,
    stack: gix::worktree::Stack,
    workdir: PathBuf,
}

impl IgnoreMatcher {
    /// Build the matcher for the repository whose worktree is `workdir`.
    pub fn open(workdir: &Path) -> Result<Self> {
        let repo = crate::handle::open(workdir)?;
        let workdir = repo
            .workdir()
            .map(Path::to_path_buf)
            .ok_or_else(|| GitError::NotARepository(workdir.to_path_buf()))?;
        let index = repo
            .index_or_empty()
            .map_err(|err| GitError::Gix(err.to_string()))?;
        let stack = repo
            .excludes(
                &index,
                None,
                gix::worktree::stack::state::ignore::Source::WorktreeThenIdMappingIfNotSkipped,
            )
            .map_err(|err| GitError::Gix(err.to_string()))?
            .detach();
        Ok(Self {
            repo,
            index,
            stack,
            workdir,
        })
    }

    /// Whether git ignores `rel`, a path relative to the worktree root. A path
    /// that no longer exists is matched as a file. Errors reading a
    /// `.gitignore` count as "not ignored".
    pub fn is_ignored(&mut self, rel: &Path) -> bool {
        if rel.as_os_str().is_empty() || self.is_tracked(rel) {
            return false;
        }
        // Checked as a file first: an ignored ancestor directory, the common
        // case (`target/…`), answers without a stat.
        if self.excluded(rel, None) {
            return true;
        }
        let is_dir = std::fs::symlink_metadata(self.workdir.join(rel))
            .map(|m| m.is_dir())
            .unwrap_or(false);
        is_dir && self.excluded(rel, Some(gix::index::entry::Mode::DIR))
    }

    fn excluded(&mut self, rel: &Path, mode: Option<gix::index::entry::Mode>) -> bool {
        self.stack
            .at_path(rel, mode, &self.repo.objects)
            .map(|platform| platform.is_excluded())
            .unwrap_or(false)
    }

    /// The index has `rel` itself or, for a directory, something below it.
    fn is_tracked(&self, rel: &Path) -> bool {
        let path = gix::path::to_unix_separators_on_windows(gix::path::into_bstr(rel));
        let path = path.as_ref();
        if self.index.entry_by_path(path).is_some() {
            return true;
        }
        let mut prefix = path.to_owned();
        prefix.push(b'/');
        self.index
            .prefixed_entries_range(prefix.as_bstr())
            .is_some_and(|range| !range.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_special_characters() {
        assert_eq!(
            escape_gitignore_pattern("a[1]!*#?.txt"),
            "a\\[1\\]\\!\\*\\#\\?.txt"
        );
        assert_eq!(escape_gitignore_pattern("src/main.rs"), "src/main.rs");
    }

    #[test]
    fn appends_with_trailing_newline() {
        let dir = tempfile::tempdir().unwrap();
        append_ignore_rules(dir.path(), &["*.log".into()], false).unwrap();
        std::fs::write(dir.path().join(".gitignore"), "*.log\nbuild").unwrap();
        append_ignore_files(dir.path(), &["dist".into(), "we!rd".into()], false).unwrap();
        let text = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert_eq!(text, "*.log\nbuild\ndist\nwe\\!rd\n");
    }

    #[test]
    fn save_normalises_and_deletes() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_gitignore(dir.path()).unwrap(), None);
        save_gitignore(dir.path(), "a\nb", false).unwrap();
        assert_eq!(
            read_gitignore(dir.path()).unwrap().as_deref(),
            Some("a\nb\n")
        );
        save_gitignore(dir.path(), "a\r\nb\n", true).unwrap();
        assert_eq!(
            read_gitignore(dir.path()).unwrap().as_deref(),
            Some("a\r\nb\r\n")
        );
        save_gitignore(dir.path(), "", false).unwrap();
        assert_eq!(read_gitignore(dir.path()).unwrap(), None);
    }

    fn git(dir: &Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    fn write(root: &Path, rel: &str, text: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn matcher_follows_git_ignore_rules() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-q"]);
        write(
            root,
            ".gitignore",
            "target/\n*.log\n/build\nlogs/*\n!logs/keep\n",
        );
        write(root, "sub/.gitignore", "gen/\n");
        write(root, ".git/info/exclude", "secret\n");
        let global = root.join(".git/global-excludes");
        std::fs::write(&global, "*.tmp\n").unwrap();
        git(
            root,
            &["config", "core.excludesFile", global.to_str().unwrap()],
        );
        write(root, "target/debug/deps/x.o", "");
        write(root, "target/keep.txt", "");
        write(root, "sub/gen/out.rs", "");
        write(root, "src/main.rs", "");
        git(root, &["add", "-f", "target/keep.txt"]);

        let mut m = IgnoreMatcher::open(root).unwrap();
        let ignored = |m: &mut IgnoreMatcher, p: &str| m.is_ignored(Path::new(p));
        // an ignored directory ignores everything below it
        assert!(ignored(&mut m, "target/debug/deps/x.o"));
        assert!(ignored(&mut m, "target/debug"));
        assert!(ignored(&mut m, "target/gone/never-existed.rs"));
        assert!(ignored(&mut m, "a.log"));
        assert!(ignored(&mut m, "src/deep/b.log"));
        assert!(ignored(&mut m, "build"));
        assert!(!ignored(&mut m, "src/build"));
        assert!(ignored(&mut m, "logs/today"));
        assert!(!ignored(&mut m, "logs/keep"));
        // nested .gitignore, info/exclude, core.excludesFile
        assert!(ignored(&mut m, "sub/gen/out.rs"));
        assert!(!ignored(&mut m, "gen/out.rs"));
        assert!(ignored(&mut m, "secret"));
        assert!(ignored(&mut m, "notes.tmp"));
        // tracked paths still count, as does a directory holding one
        assert!(!ignored(&mut m, "target/keep.txt"));
        assert!(!ignored(&mut m, "target"));
        assert!(!ignored(&mut m, "src/main.rs"));
        assert!(!ignored(&mut m, ".gitignore"));
        assert!(!ignored(&mut m, ""));
    }

    #[test]
    fn ignores_and_untracks_oversized_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-q"]);
        write(root, "big.bin", "x");
        write(root, "new.iso", "y");
        write(root, "we[ird].dat", "z");
        git(root, &["add", "big.bin", "we[ird].dat"]);
        let bin = Arc::new(crate::detect::find_git().unwrap());
        let paths = ["big.bin", "new.iso", "we[ird].dat"].map(String::from);
        assert_eq!(
            tracked_paths(bin.clone(), root, &paths).unwrap(),
            vec!["big.bin", "we[ird].dat"]
        );
        ignore_and_untrack(bin.clone(), root, &paths, true, true).unwrap();
        assert!(tracked_paths(bin.clone(), root, &paths).unwrap().is_empty());
        assert!(root.join("big.bin").exists());
        let text = std::fs::read_to_string(root.join(".gitignore")).unwrap();
        assert_eq!(text, "big.bin\nnew.iso\nwe\\[ird\\].dat\n");
    }

    #[test]
    fn skips_patterns_already_there() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "*.log\r\nbuild\r\n").unwrap();
        let patterns = ["build".into(), "dist".into(), "dist".into(), "*.log".into()];
        append_ignore_rules(dir.path(), &patterns, true).unwrap();
        let text = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert_eq!(text, "*.log\r\nbuild\r\ndist\r\n");
        // nothing new: the file is left alone
        append_ignore_rules(dir.path(), &["build".into()], true).unwrap();
        let again = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert_eq!(again, text);
        // GHD behaviour: appended again
        append_ignore_rules(dir.path(), &["build".into()], false).unwrap();
        let text = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert_eq!(text, "*.log\r\nbuild\r\ndist\r\nbuild\r\n");
    }

    #[test]
    fn targets_and_their_patterns() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("sub/deep")).unwrap();
        std::fs::write(dir.path().join("sub/.gitignore"), "").unwrap();
        assert_eq!(
            gitignore_dirs_above(dir.path(), "sub/deep/a[1].txt"),
            vec!["sub"]
        );
        assert!(gitignore_dirs_above(dir.path(), "top.txt").is_empty());
        let path = "sub/deep/a[1].txt";
        assert_eq!(
            IgnoreTarget::Root.pattern_for(path),
            "sub/deep/a\\[1\\].txt"
        );
        assert_eq!(
            IgnoreTarget::Directory("sub".into()).pattern_for(path),
            "/deep/a\\[1\\].txt"
        );
        assert_eq!(IgnoreTarget::ExcludesFile.pattern_for(path), "a\\[1\\].txt");
    }

    #[test]
    fn writes_info_exclude() {
        let dir = tempfile::tempdir().unwrap();
        assert!(
            std::process::Command::new("git")
                .args(["init", "-q"])
                .current_dir(dir.path())
                .status()
                .unwrap()
                .success()
        );
        let git = Arc::new(crate::find_git().unwrap());
        append_ignore_rules_to(
            git,
            dir.path(),
            &IgnoreTarget::InfoExclude,
            &["local.txt".into()],
            true,
        )
        .unwrap();
        let text = std::fs::read_to_string(dir.path().join(".git/info/exclude")).unwrap();
        assert!(text.ends_with("local.txt\n"));
    }

    #[test]
    fn keeps_crlf() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "a\r\n").unwrap();
        append_ignore_rules(dir.path(), &["b".into()], false).unwrap();
        let text = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert_eq!(text, "a\r\nb\r\n");
    }
}
