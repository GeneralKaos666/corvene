//! `GIT_ASKPASS` on Android. On the desktop git runs the Corvene binary
//! itself (`crates/corvene/src/askpass.rs`), which reads the keychain. An
//! Android app has no binary to run and only the app's own process can use
//! the Android Keystore, so git runs this helper instead (packaged as
//! `libcorvene-askpass.so`), which hands the prompt to the running app over
//! the Unix socket named by `CORVENE_ASKPASS_SOCKET` and prints its answer.
//!
//! Request: the `CORVENE_ASKPASS_LOGINS` value, a newline, the prompt, a
//! newline. Reply: the answer (empty when Corvene knows nothing, which makes
//! git fail with an authentication error the app turns into its dialog).

#[cfg(unix)]
fn ask(prompt: &str) -> Option<String> {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;

    let socket = std::env::var_os("CORVENE_ASKPASS_SOCKET")?;
    let logins = std::env::var("CORVENE_ASKPASS_LOGINS").unwrap_or_default();
    let mut stream = UnixStream::connect(socket).ok()?;
    // one line each: neither may carry a line break
    let request = format!(
        "{}\n{}\n",
        logins.replace(['\r', '\n'], " "),
        prompt.replace(['\r', '\n'], " ")
    );
    stream.write_all(request.as_bytes()).ok()?;
    stream.shutdown(std::net::Shutdown::Write).ok()?;
    let mut answer = String::new();
    stream.read_to_string(&mut answer).ok()?;
    Some(answer)
}

/// Only built there so that the workspace builds: Windows git runs the
/// Corvene binary itself.
#[cfg(not(unix))]
fn ask(_prompt: &str) -> Option<String> {
    None
}

fn main() {
    let prompt = std::env::args().nth(1).unwrap_or_default();
    println!("{}", ask(&prompt).unwrap_or_default());
}
