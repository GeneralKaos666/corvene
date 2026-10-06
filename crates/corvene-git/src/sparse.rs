//! Corvene `1112-sparse-checkout`: cone-mode sparse checkout (`git
//! sparse-checkout`). GHD has none: every file of the tree is checked out.

use std::path::Path;
use std::sync::Arc;

use crate::detect::GitBinary;
use crate::error::Result;
use crate::process::GitCommand;

/// The repository's sparse checkout settings.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SparseCheckout {
    /// `core.sparseCheckout`.
    pub enabled: bool,
    /// `core.sparseCheckoutCone`: the patterns are folders.
    pub cone: bool,
    /// `git sparse-checkout list`: the folders checked out (cone mode, the
    /// files at the top are always in), or the patterns otherwise.
    pub patterns: Vec<String>,
}

/// `core.sparseCheckout`, read without listing the patterns (the repository
/// refresh asks every time).
pub fn sparse_checkout_enabled(git: Arc<GitBinary>, workdir: &Path) -> bool {
    crate::config::boolean_config_value(git, workdir, "core.sparseCheckout", false).unwrap_or(false)
}

/// The sparse checkout settings; the patterns only while it is on.
pub fn sparse_checkout(git: Arc<GitBinary>, workdir: &Path) -> Result<SparseCheckout> {
    if !sparse_checkout_enabled(git.clone(), workdir) {
        return Ok(SparseCheckout::default());
    }
    let cone =
        crate::config::boolean_config_value(git.clone(), workdir, "core.sparseCheckoutCone", false)
            .unwrap_or(false);
    let out = GitCommand::new(git)
        .args(["sparse-checkout", "list"])
        .current_dir(workdir)
        .run()?;
    let patterns = out
        .stdout_string()?
        .lines()
        .filter(|l| !l.is_empty())
        .map(crate::clean::unquote_c_path)
        .collect();
    Ok(SparseCheckout {
        enabled: true,
        cone,
        patterns,
    })
}

/// Every folder of `HEAD`'s tree (`git ls-tree -r -d --name-only`), parents
/// before their children. Empty for an unborn branch.
pub fn tree_directories(git: Arc<GitBinary>, workdir: &Path) -> Result<Vec<String>> {
    let out = GitCommand::new(git)
        .args(["ls-tree", "-r", "-d", "-z", "--name-only", "HEAD"])
        .current_dir(workdir)
        .allow_exit_code(128)
        .run()?;
    if !out.status.success() {
        return Ok(Vec::new());
    }
    Ok(out
        .stdout
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .map(|p| String::from_utf8_lossy(p).into_owned())
        .collect())
}

/// `git sparse-checkout set --cone --stdin`: check out the files at the top
/// and the `folders` (with everything in them); no folders leaves only the
/// files at the top. Turns sparse checkout on when it is off.
pub fn sparse_checkout_set(git: Arc<GitBinary>, workdir: &Path, folders: &[String]) -> Result<()> {
    let mut input = folders.join("\n");
    input.push('\n');
    GitCommand::new(git)
        .args(["sparse-checkout", "set", "--cone", "--stdin"])
        .stdin(input.into_bytes())
        .current_dir(workdir)
        .run()?;
    Ok(())
}

/// `git sparse-checkout disable`: every file is checked out again.
pub fn sparse_checkout_disable(git: Arc<GitBinary>, workdir: &Path) -> Result<()> {
    GitCommand::new(git)
        .args(["sparse-checkout", "disable"])
        .current_dir(workdir)
        .run()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sets_lists_and_disables_a_cone() {
        use std::process::Command;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let run = |args: &[&str]| {
            assert!(
                Command::new("git")
                    .args(args)
                    .current_dir(root)
                    .status()
                    .unwrap()
                    .success()
            )
        };
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "commit.gpgsign", "false"]);
        run(&["config", "user.name", "T"]);
        run(&["config", "user.email", "t@example.com"]);
        for file in [
            "top.txt",
            "src/a.rs",
            "src/deep/b.rs",
            "docs/guide.md",
            "my dir/x",
        ] {
            let p = root.join(file);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, "x\n").unwrap();
        }
        run(&["add", "-A"]);
        run(&["commit", "-q", "-m", "files"]);
        let git = Arc::new(crate::find_git().unwrap());
        assert_eq!(
            tree_directories(git.clone(), root).unwrap(),
            vec!["docs", "my dir", "src", "src/deep"]
        );
        assert!(!sparse_checkout(git.clone(), root).unwrap().enabled);

        sparse_checkout_set(git.clone(), root, &["src/deep".into(), "my dir".into()]).unwrap();
        let sparse = sparse_checkout(git.clone(), root).unwrap();
        assert!(sparse.enabled && sparse.cone);
        assert_eq!(sparse.patterns, vec!["my dir", "src/deep"]);
        assert!(root.join("top.txt").exists());
        assert!(root.join("src/deep/b.rs").exists());
        assert!(!root.join("docs/guide.md").exists());

        sparse_checkout_set(git.clone(), root, &[]).unwrap();
        assert!(
            sparse_checkout(git.clone(), root)
                .unwrap()
                .patterns
                .is_empty()
        );
        assert!(!root.join("src").exists());

        sparse_checkout_disable(git.clone(), root).unwrap();
        assert!(!sparse_checkout_enabled(git.clone(), root));
        assert!(root.join("docs/guide.md").exists());
    }
}
