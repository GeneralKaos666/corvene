//! Ref helpers - GHD `lib/git/refs.ts` (`formatAsLocalRef`,
//! `getSymbolicRef`) and `lib/git/update-ref.ts` (`deleteRef`).

use std::path::Path;
use std::sync::Arc;

use crate::detect::GitBinary;
use crate::error::Result;
use crate::process::GitCommand;

/// GHD `formatAsLocalRef`: a local branch name as a fully qualified ref.
/// `main` → `refs/heads/main`, `heads/Microsoft/main` (git's spelling when a
/// remote ref has the same short name) → `refs/heads/Microsoft/main`, and
/// `refs/heads/main` stays as it is.
pub fn format_as_local_ref(name: &str) -> String {
    if name.starts_with("heads/") {
        format!("refs/{name}")
    } else if !name.starts_with("refs/heads/") {
        format!("refs/heads/{name}")
    } else {
        name.to_string()
    }
}

/// GHD `getSymbolicRef`: the ref `reference` points at (`git symbolic-ref
/// -q <ref>`), `None` when it is missing (exit 128) or not symbolic (exit 1).
pub fn get_symbolic_ref(
    git: Arc<GitBinary>,
    workdir: &Path,
    reference: &str,
) -> Result<Option<String>> {
    let out = GitCommand::new(git)
        .args(["symbolic-ref", "-q", reference])
        .current_dir(workdir)
        .allow_exit_code(1)
        .allow_exit_code(128)
        .run()?;
    if !out.status.success() {
        return Ok(None);
    }
    Ok(Some(out.stdout_string()?.trim().to_string()))
}

/// GHD `deleteRef`: `git update-ref -d <ref> [-m <reason>]`. `reason` only
/// matters for `HEAD`; deleting any other ref drops its reflog with it.
pub fn delete_ref(
    git: Arc<GitBinary>,
    workdir: &Path,
    reference: &str,
    reason: Option<&str>,
) -> Result<()> {
    let mut cmd = GitCommand::new(git)
        .args(["update-ref", "-d", reference])
        .current_dir(workdir);
    if let Some(reason) = reason {
        cmd = cmd.args(["-m", reason]);
    }
    cmd.run()?;
    Ok(())
}
