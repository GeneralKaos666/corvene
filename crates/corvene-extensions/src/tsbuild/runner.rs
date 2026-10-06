//! Running the compiler: no shell, a wall-clock timeout, both output
//! streams kept in the build log.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Run `program args` in `cwd`; stdout and stderr go to `log`. The error
/// holds the last lines of output.
pub fn run(
    program: &Path,
    args: &[String],
    cwd: &Path,
    timeout: Duration,
    log: &mut String,
) -> Result<(), String> {
    log.push_str(&format!("$ {} {}\n", program.display(), args.join(" ")));
    let started = Instant::now();
    let span = tracing::info_span!("tsbuild", program = %program.display());
    let _enter = span.enter();
    let mut child = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("could not run {}: {err}", program.display()))?;
    // drain both pipes on threads so a chatty compiler never blocks
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let out_thread = std::thread::spawn(move || {
        let mut buf = String::new();
        if let Some(s) = stdout.as_mut() {
            let _ = s.read_to_string(&mut buf);
        }
        buf
    });
    let err_thread = std::thread::spawn(move || {
        let mut buf = String::new();
        if let Some(s) = stderr.as_mut() {
            let _ = s.read_to_string(&mut buf);
        }
        buf
    });
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if started.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(format!(
                    "{} took longer than {} s",
                    program.display(),
                    timeout.as_secs()
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(err) => break Err(err.to_string()),
        }
    };
    let out = out_thread.join().unwrap_or_default();
    let err = err_thread.join().unwrap_or_default();
    log.push_str(&out);
    log.push_str(&err);
    let status = status?;
    log.push_str(&format!(
        "(exit {}, {:.1} s)\n",
        status.code().unwrap_or(-1),
        started.elapsed().as_secs_f32()
    ));
    if status.success() {
        return Ok(());
    }
    let tail: Vec<&str> = err.lines().chain(out.lines()).rev().take(20).collect();
    let tail: Vec<&str> = tail.into_iter().rev().collect();
    Err(format!(
        "{} failed (exit {}):\n{}",
        program
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        status.code().unwrap_or(-1),
        tail.join("\n")
    ))
}

// the test drives /bin/sh
#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn captures_output_and_failures() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut log = String::new();
        run(
            Path::new("/bin/sh"),
            &["-c".into(), "echo hi; echo err >&2".into()],
            dir.path(),
            Duration::from_secs(5),
            &mut log,
        )
        .expect("ok");
        assert!(log.contains("hi") && log.contains("err"), "{log}");
        let err = run(
            Path::new("/bin/sh"),
            &["-c".into(), "echo boom >&2; exit 3".into()],
            dir.path(),
            Duration::from_secs(5),
            &mut log,
        )
        .expect_err("fails");
        assert!(err.contains("exit 3") && err.contains("boom"), "{err}");
        let err = run(
            Path::new("/bin/sh"),
            &["-c".into(), "sleep 5".into()],
            dir.path(),
            Duration::from_millis(200),
            &mut log,
        )
        .expect_err("times out");
        assert!(err.contains("longer"), "{err}");
    }
}
