//! Starting git on Android without `fork`.
//!
//! `std::process::Command` forks on Android (it uses `posix_spawn` only with
//! glibc and musl). The application's process is large (the Java runtime,
//! the GPU driver's mappings), so every fork copies its page tables and
//! holds the address-space lock while the other threads wait: on a phone a
//! `git config --get` that takes 20 ms from a shell took 100 ms from
//! Corvene, and a refresh runs eight of them at once. bionic's
//! `posix_spawn` with `POSIX_SPAWN_USEVFORK` shares the address space until
//! `exec` instead.
//!
//! `posix_spawn` appeared in API 28 and Corvene runs from API 26, so the
//! functions are looked up at run time; without them [`spawn`] falls back to
//! `Command`. The working directory becomes `git -C <dir>` (bionic has no
//! `posix_spawn_file_actions_addchdir_np` before API 34).
//!
//! Compiled for tests on every Unix so the path is exercised on the host.

use std::ffi::{CString, OsString, c_char, c_int, c_short, c_void};
use std::fs::File;
use std::io::{self, Read};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, ExitStatus, Output, Stdio};
use std::sync::OnceLock;

/// A running git: the parts of `std::process::Child` that `process.rs` uses.
pub struct Child {
    pid: libc::pid_t,
    pub stdin: Option<File>,
    pub stdout: Option<File>,
    pub stderr: Option<File>,
}

impl Child {
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub fn id(&self) -> u32 {
        self.pid as u32
    }

    /// `std::process::Child::kill`: SIGKILL; the caller still waits.
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub fn kill(&mut self) -> io::Result<()> {
        // SAFETY: signalling our own child by pid
        if unsafe { libc::kill(self.pid, libc::SIGKILL) } == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    pub fn wait(&mut self) -> io::Result<ExitStatus> {
        drop(self.stdin.take());
        let mut status: c_int = 0;
        loop {
            // SAFETY: `waitpid` writes the status of our own child
            let result = unsafe { libc::waitpid(self.pid, &mut status, 0) };
            if result != -1 {
                return Ok(ExitStatus::from_raw(status));
            }
            let err = io::Error::last_os_error();
            if err.kind() != io::ErrorKind::Interrupted {
                return Err(err);
            }
        }
    }

    /// Reads both pipes to their end (stderr on a thread, so neither fills
    /// up while the other is read), then waits.
    pub fn wait_with_output(mut self) -> io::Result<Output> {
        drop(self.stdin.take());
        let stderr = self.stderr.take();
        let reader = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            if let Some(mut stderr) = stderr {
                let _ = stderr.read_to_end(&mut bytes);
            }
            bytes
        });
        let mut stdout = Vec::new();
        if let Some(mut pipe) = self.stdout.take() {
            let _ = pipe.read_to_end(&mut stdout);
        }
        let stderr = reader.join().unwrap_or_default();
        let status = self.wait()?;
        Ok(Output {
            status,
            stdout,
            stderr,
        })
    }
}

/// Storage for a `posix_spawnattr_t` or `posix_spawn_file_actions_t`: a
/// pointer on bionic and macOS, a structure of up to 336 bytes on glibc.
#[repr(C, align(16))]
struct Opaque([u8; 512]);

type Init = unsafe extern "C" fn(*mut Opaque) -> c_int;

struct Functions {
    spawn: unsafe extern "C" fn(
        *mut libc::pid_t,
        *const c_char,
        *const Opaque,
        *const Opaque,
        *const *mut c_char,
        *const *mut c_char,
    ) -> c_int,
    actions_init: Init,
    actions_destroy: Init,
    actions_adddup2: unsafe extern "C" fn(*mut Opaque, c_int, c_int) -> c_int,
    actions_addopen:
        unsafe extern "C" fn(*mut Opaque, c_int, *const c_char, c_int, libc::mode_t) -> c_int,
    attr_init: Init,
    attr_destroy: Init,
    attr_setflags: unsafe extern "C" fn(*mut Opaque, c_short) -> c_int,
    attr_setsigdefault: unsafe extern "C" fn(*mut Opaque, *const libc::sigset_t) -> c_int,
    attr_setsigmask: unsafe extern "C" fn(*mut Opaque, *const libc::sigset_t) -> c_int,
}

/// The `posix_spawn` family of this C library, when it has one.
// the fields of `Functions` give each transmute its target type
#[allow(clippy::missing_transmute_annotations)]
fn functions() -> Option<&'static Functions> {
    static FUNCTIONS: OnceLock<Option<Functions>> = OnceLock::new();
    FUNCTIONS
        .get_or_init(|| {
            let lookup = |name: &std::ffi::CStr| {
                // SAFETY: a symbol lookup in the libraries already loaded
                let symbol = unsafe { libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr()) };
                (!symbol.is_null()).then_some(symbol)
            };
            // SAFETY: each symbol is the C function of that name, whose
            // signature POSIX fixes
            unsafe {
                Some(Functions {
                    spawn: std::mem::transmute::<*mut c_void, _>(lookup(c"posix_spawn")?),
                    actions_init: std::mem::transmute::<*mut c_void, _>(lookup(
                        c"posix_spawn_file_actions_init",
                    )?),
                    actions_destroy: std::mem::transmute::<*mut c_void, _>(lookup(
                        c"posix_spawn_file_actions_destroy",
                    )?),
                    actions_adddup2: std::mem::transmute::<*mut c_void, _>(lookup(
                        c"posix_spawn_file_actions_adddup2",
                    )?),
                    actions_addopen: std::mem::transmute::<*mut c_void, _>(lookup(
                        c"posix_spawn_file_actions_addopen",
                    )?),
                    attr_init: std::mem::transmute::<*mut c_void, _>(lookup(
                        c"posix_spawnattr_init",
                    )?),
                    attr_destroy: std::mem::transmute::<*mut c_void, _>(lookup(
                        c"posix_spawnattr_destroy",
                    )?),
                    attr_setflags: std::mem::transmute::<*mut c_void, _>(lookup(
                        c"posix_spawnattr_setflags",
                    )?),
                    attr_setsigdefault: std::mem::transmute::<*mut c_void, _>(lookup(
                        c"posix_spawnattr_setsigdefault",
                    )?),
                    attr_setsigmask: std::mem::transmute::<*mut c_void, _>(lookup(
                        c"posix_spawnattr_setsigmask",
                    )?),
                })
            }
        })
        .as_ref()
}

const POSIX_SPAWN_SETSIGDEF: c_short = 0x04;
const POSIX_SPAWN_SETSIGMASK: c_short = 0x08;
/// bionic and glibc: `vfork` instead of `fork`. Other libraries (macOS in
/// the tests) do not know the flag and never fork anyway.
const POSIX_SPAWN_USEVFORK: c_short = if cfg!(any(target_os = "android", target_os = "linux")) {
    0x40
} else {
    0
};

fn c_string(bytes: impl Into<Vec<u8>>) -> io::Result<CString> {
    CString::new(bytes).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL in argument"))
}

fn check(result: c_int) -> io::Result<()> {
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(result))
    }
}

/// Starts what `command` describes (program, arguments, working directory
/// and environment changes) with stdout and stderr piped, and stdin piped
/// or `/dev/null`.
pub fn spawn(mut command: Command, pipe_stdin: bool) -> io::Result<Child> {
    let Some(functions) = functions() else {
        return spawn_with_std(&mut command, pipe_stdin);
    };

    let mut argv = vec![c_string(command.get_program().as_bytes())?];
    if let Some(dir) = command.get_current_dir() {
        argv.push(c_string("-C")?);
        argv.push(c_string(dir.as_os_str().as_bytes())?);
    }
    for arg in command.get_args() {
        argv.push(c_string(arg.as_bytes())?);
    }
    let mut env: Vec<(OsString, OsString)> = std::env::vars_os().collect();
    for (key, value) in command.get_envs() {
        env.retain(|(existing, _)| existing != key);
        if let Some(value) = value {
            env.push((key.to_os_string(), value.to_os_string()));
        }
    }
    let envp = env
        .into_iter()
        .map(|(key, value)| {
            let mut entry = key.into_vec();
            entry.push(b'=');
            entry.extend_from_slice(value.as_bytes());
            c_string(entry)
        })
        .collect::<io::Result<Vec<_>>>()?;
    let pointers = |strings: &[CString]| -> Vec<*mut c_char> {
        strings
            .iter()
            .map(|s| s.as_ptr().cast_mut())
            .chain(std::iter::once(std::ptr::null_mut()))
            .collect()
    };
    let (argv_pointers, envp_pointers) = (pointers(&argv), pointers(&envp));

    // close-on-exec pipes: the child keeps only what `dup2` puts on 0, 1, 2
    let (stdout_read, stdout_write) = io::pipe()?;
    let (stderr_read, stderr_write) = io::pipe()?;
    let stdin = if pipe_stdin { Some(io::pipe()?) } else { None };
    let dev_null = c_string("/dev/null")?;

    let mut actions = Opaque([0; 512]);
    let mut attr = Opaque([0; 512]);
    let mut pid: libc::pid_t = 0;
    // SAFETY: the actions and attributes are initialised before use and
    // destroyed after; every pointer handed over outlives the call
    let result = unsafe {
        check((functions.actions_init)(&mut actions))?;
        if let Err(err) = check((functions.attr_init)(&mut attr)) {
            (functions.actions_destroy)(&mut actions);
            return Err(err);
        }
        let prepared = (|| {
            match &stdin {
                Some((read, _)) => check((functions.actions_adddup2)(
                    &mut actions,
                    read.as_raw_fd(),
                    0,
                ))?,
                None => check((functions.actions_addopen)(
                    &mut actions,
                    0,
                    dev_null.as_ptr(),
                    libc::O_RDONLY,
                    0,
                ))?,
            }
            check((functions.actions_adddup2)(
                &mut actions,
                stdout_write.as_raw_fd(),
                1,
            ))?;
            check((functions.actions_adddup2)(
                &mut actions,
                stderr_write.as_raw_fd(),
                2,
            ))?;
            // as `Command` does: no blocked signals, and SIGPIPE (which the
            // Rust runtime ignores) back to its default
            let mut none = std::mem::zeroed::<libc::sigset_t>();
            let mut defaults = std::mem::zeroed::<libc::sigset_t>();
            libc::sigemptyset(&mut none);
            libc::sigemptyset(&mut defaults);
            libc::sigaddset(&mut defaults, libc::SIGPIPE);
            check((functions.attr_setsigmask)(&mut attr, &none))?;
            check((functions.attr_setsigdefault)(&mut attr, &defaults))?;
            check((functions.attr_setflags)(
                &mut attr,
                POSIX_SPAWN_SETSIGMASK | POSIX_SPAWN_SETSIGDEF | POSIX_SPAWN_USEVFORK,
            ))
        })();
        let result = prepared.and_then(|()| {
            check((functions.spawn)(
                &mut pid,
                argv[0].as_ptr(),
                &actions,
                &attr,
                argv_pointers.as_ptr(),
                envp_pointers.as_ptr(),
            ))
        });
        (functions.actions_destroy)(&mut actions);
        (functions.attr_destroy)(&mut attr);
        result
    };
    result?;
    Ok(Child {
        pid,
        stdin: stdin.map(|(_, write)| File::from(OwnedFd::from(write))),
        stdout: Some(File::from(OwnedFd::from(stdout_read))),
        stderr: Some(File::from(OwnedFd::from(stderr_read))),
    })
}

/// Android 8: no `posix_spawn`, so `Command` forks.
fn spawn_with_std(command: &mut Command, pipe_stdin: bool) -> io::Result<Child> {
    let mut child = command
        .stdin(if pipe_stdin {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    Ok(Child {
        pid: child.id() as libc::pid_t,
        stdin: child
            .stdin
            .take()
            .map(|pipe| File::from(OwnedFd::from(pipe))),
        stdout: child
            .stdout
            .take()
            .map(|pipe| File::from(OwnedFd::from(pipe))),
        stderr: child
            .stderr
            .take()
            .map(|pipe| File::from(OwnedFd::from(pipe))),
    })
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn git() -> Command {
        Command::new(
            [
                "/usr/bin/git",
                "/opt/homebrew/bin/git",
                "/usr/local/bin/git",
            ]
            .into_iter()
            .find(|path| std::path::Path::new(path).exists())
            .unwrap_or("/usr/bin/git"),
        )
    }

    #[test]
    fn captures_output_and_status() {
        assert!(functions().is_some());
        let mut command = git();
        command.arg("--version");
        let output = spawn(command, false).unwrap().wait_with_output().unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).starts_with("git version"));

        let mut command = git();
        command.arg("no-such-command");
        let output = spawn(command, false).unwrap().wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).contains("not a git command"));
    }

    #[test]
    fn runs_in_the_working_directory_with_the_environment() {
        let dir = tempfile::tempdir().unwrap();
        let mut init = git();
        init.args(["init", "-q"]).current_dir(dir.path());
        assert!(spawn(init, false).unwrap().wait().unwrap().success());
        assert!(dir.path().join(".git").is_dir());

        let mut command = git();
        command
            .args(["var", "GIT_AUTHOR_IDENT"])
            .current_dir(dir.path())
            .env("GIT_AUTHOR_NAME", "Spawn Test")
            .env("GIT_AUTHOR_EMAIL", "spawn@example.com")
            .env_remove("GIT_AUTHOR_DATE");
        let output = spawn(command, false).unwrap().wait_with_output().unwrap();
        assert!(
            String::from_utf8_lossy(&output.stdout).starts_with("Spawn Test <spawn@example.com>")
        );
    }

    #[test]
    fn feeds_stdin() {
        let dir = tempfile::tempdir().unwrap();
        let mut command = git();
        command
            .args(["hash-object", "--stdin"])
            .current_dir(dir.path());
        let mut child = spawn(command, true).unwrap();
        child.stdin.take().unwrap().write_all(b"hello\n").unwrap();
        let output = child.wait_with_output().unwrap();
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "ce013625030ba8dba906f756967f9e9ca394464a"
        );
    }

    #[test]
    fn falls_back_to_command() {
        let mut command = git();
        command.arg("--version");
        let child = spawn_with_std(&mut command, false).unwrap();
        assert!(child.wait_with_output().unwrap().status.success());
    }
}
