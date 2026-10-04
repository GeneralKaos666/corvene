//! Running git from tests: the binary every test shares and dugite's
//! `exec`, on top of `corvene_git::process::GitCommand` (the only place
//! that spawns git).

use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::sync::{Arc, OnceLock};

use corvene_git::{GitBinary, GitCommand};

use crate::env::{home_dir, init};

/// The git binary every test uses (`corvene_git::find_git`, looked up once
/// per process). Pass it to `corvene_git` functions: `git()` for each call.
///
/// # Panics
///
/// When no usable git is installed.
pub fn git() -> Arc<GitBinary> {
    static GIT: OnceLock<Arc<GitBinary>> = OnceLock::new();
    init();
    GIT.get_or_init(|| {
        Arc::new(corvene_git::find_git().expect("corvene-test-support: no usable git found"))
    })
    .clone()
}

/// A [`GitCommand`] for the shared [`git`] binary running in `cwd`, for
/// tests that need its builder (streaming, `env_remove`, raw stdout bytes).
pub fn git_command(cwd: impl AsRef<Path>) -> GitCommand {
    GitCommand::new(git()).current_dir(cwd)
}

/// What dugite's `exec` resolves to (`IGitResult`): the output of a git
/// invocation whatever its exit code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecResult {
    /// stdout, decoded as UTF-8 (lossily).
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// dugite's `IGitExecutionOptions` as GitHub Desktop's tests use them.
#[derive(Clone, Debug, Default)]
pub struct ExecOptions {
    /// Written to git's stdin (`exec(['apply'], path, { stdin: patch })`).
    pub stdin: Option<Vec<u8>>,
    /// Extra environment variables for this invocation only.
    pub env: Vec<(OsString, OsString)>,
}

/// dugite's `exec(args, path)`: run git in `cwd` and return its output. A
/// non-zero exit code is not a failure (tests assert on
/// [`ExecResult::exit_code`]).
///
/// # Panics
///
/// When git cannot be started or is killed by a signal.
pub fn exec<I, S>(args: I, cwd: impl AsRef<Path>) -> ExecResult
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    exec_with(args, cwd, ExecOptions::default())
}

/// dugite's `exec(args, path, options)` (see [`exec`]).
///
/// # Panics
///
/// When git cannot be started or is killed by a signal.
pub fn exec_with<I, S>(args: I, cwd: impl AsRef<Path>, options: ExecOptions) -> ExecResult
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let cwd = cwd.as_ref();
    let mut command = git_command(cwd).args(args).allow_any_exit_code();
    if let Some(stdin) = options.stdin {
        command = command.stdin(stdin);
    }
    for (key, value) in options.env {
        command = command.env(key, value);
    }
    let output = command
        .run()
        .unwrap_or_else(|err| panic!("git in {}: {err}", cwd.display()));
    ExecResult {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: output.stderr,
        exit_code: output
            .status
            .code()
            .expect("allow_any_exit_code only returns output for exited processes"),
    }
}

/// GitHub Desktop's `git(args, path, name)` (`lib/git/core.ts`) as its
/// helpers use it: like [`exec`], but any exit code other than 0 is a
/// failure.
///
/// # Panics
///
/// When git cannot be started or exits with a non-zero code.
pub fn exec_ok<I, S>(args: I, cwd: impl AsRef<Path>) -> ExecResult
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let args: Vec<OsString> = args
        .into_iter()
        .map(|a| a.as_ref().to_os_string())
        .collect();
    let cwd = cwd.as_ref();
    let result = exec(&args, cwd);
    assert_eq!(
        result.exit_code,
        0,
        "`git {}` in {} failed: {}",
        args.iter()
            .map(|a| a.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" "),
        cwd.display(),
        result.stderr.trim()
    );
    result
}

/// Whether the test git runs `git lfs` (GitHub Desktop's tests take it for
/// granted: dugite's git bundles Git LFS; a system git may lack it). Probed
/// once per process with `git lfs version`. The cases that need it run
/// unconditionally (the CI runners have `git-lfs`) and fail with a clear
/// message without it.
pub fn has_git_lfs() -> bool {
    static HAS_GIT_LFS: OnceLock<bool> = OnceLock::new();
    *HAS_GIT_LFS.get_or_init(|| exec(["lfs", "version"], home_dir()).exit_code == 0)
}
