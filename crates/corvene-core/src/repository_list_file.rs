//! Corvene (`425-cli-list-repositories`): the repository list for other
//! programs. The store is locked while Corvene runs, so whenever the list
//! or the selection is saved Corvene also writes `<data dir>/repositories.json`
//! (`[{"name", "path", "selected"}]`) and `<data dir>/repositories.txt` (one
//! `* name<TAB>path` line per repository, `*` marking the selected one,
//! else two spaces), which the `corvene list [--json]` command prints. GHD
//! has no way to read its repository list from outside.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use corvene_models::Repository;
use serde::Serialize;
use tracing::warn;

static ENABLED: AtomicBool = AtomicBool::new(false);

pub const JSON_FILE: &str = "repositories.json";
pub const TEXT_FILE: &str = "repositories.txt";

/// One entry of `repositories.json`.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct ListedRepository {
    pub name: String,
    pub path: String,
    pub selected: bool,
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Turn the files on (written at the next save, and now when `now` has the
/// list) or off (removed from `dir`).
pub fn set_enabled(on: bool, dir: &Path, now: Option<(&[Repository], Option<u64>)>) {
    ENABLED.store(on, Ordering::Relaxed);
    if !on {
        let _ = std::fs::remove_file(dir.join(JSON_FILE));
        let _ = std::fs::remove_file(dir.join(TEXT_FILE));
    } else if let Some((repositories, selected)) = now {
        write(dir, repositories, selected);
    }
}

/// The entries for `repositories`, by name ignoring case.
pub fn listed(repositories: &[Repository], selected: Option<u64>) -> Vec<ListedRepository> {
    let mut listed: Vec<ListedRepository> = repositories
        .iter()
        .map(|r| ListedRepository {
            name: r.name(),
            path: r.path.to_string_lossy().into_owned(),
            selected: selected == Some(r.id),
        })
        .collect();
    listed.sort_by_cached_key(|r| r.name.to_lowercase());
    listed
}

/// `repositories.txt`'s text.
pub fn text(listed: &[ListedRepository]) -> String {
    listed
        .iter()
        .map(|r| {
            format!(
                "{} {}\t{}\n",
                if r.selected { "*" } else { " " },
                r.name,
                r.path
            )
        })
        .collect()
}

/// Write both files to `dir` (each through a temporary file, so a reader
/// never sees half of one) while enabled.
pub fn write(dir: &Path, repositories: &[Repository], selected: Option<u64>) {
    if !enabled() {
        return;
    }
    let listed = listed(repositories, selected);
    let json = match serde_json::to_vec_pretty(&listed) {
        Ok(mut json) => {
            json.push(b'\n');
            json
        }
        Err(err) => {
            warn!(%err, "could not write the repository list file");
            return;
        }
    };
    for (name, bytes) in [(JSON_FILE, json), (TEXT_FILE, text(&listed).into_bytes())] {
        let tmp = dir.join(format!("{name}.tmp"));
        if let Err(err) =
            std::fs::write(&tmp, bytes).and_then(|()| std::fs::rename(&tmp, dir.join(name)))
        {
            warn!(%err, file = name, "could not write the repository list file");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_files_list_every_repository_and_mark_the_selected_one() {
        let dir = tempfile::tempdir().unwrap();
        let mut b = Repository::new(2, "/code/beta");
        b.alias = Some("Beta app".into());
        let repos = vec![Repository::new(1, "/code/zeta"), b];
        set_enabled(true, dir.path(), Some((&repos, Some(1))));
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.path().join(JSON_FILE)).unwrap()).unwrap();
        assert_eq!(
            json,
            serde_json::json!([
                {"name": "Beta app", "path": "/code/beta", "selected": false},
                {"name": "zeta", "path": "/code/zeta", "selected": true},
            ])
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join(TEXT_FILE)).unwrap(),
            "  Beta app\t/code/beta\n* zeta\t/code/zeta\n"
        );
        set_enabled(false, dir.path(), None);
        assert!(!dir.path().join(JSON_FILE).exists());
        assert!(!dir.path().join(TEXT_FILE).exists());
    }
}
