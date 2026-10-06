//! Corvene `1109-remote-manager`: Repository Settings › Remote lists every
//! remote, edits them and picks where pushes go. GitHub Desktop's Remote tab
//! (`ui/repository-settings/remote.tsx`) only edits the primary remote's
//! URL, and its push (`lib/git/push.ts`) always names the upstream's remote,
//! so git's `remote.pushDefault` and `branch.<name>.pushRemote` are ignored
//! (desktop#18154).
//!
//! The dialog stages its changes as [`RemoteEdit`]s and writes them on Save
//! ([`apply_remote_edits`]), then the push settings ([`PushTargetConfig`]).
//! The push remote in effect for the current branch (its `pushRemote`, else
//! `remote.pushDefault`) becomes [`RepositoryState::push_target`] on each
//! refresh when it differs from the upstream's remote: Push sends the branch
//! to the same-named branch there, the toolbar counts the commits that
//! branch lacks, Pull keeps following the upstream, and a branch without an
//! upstream is published to it. Branch › Push To ▸ (`1210`) checks that
//! remote.
//!
//! [`RepositoryState::push_target`]: crate::state::RepositoryState::push_target

use std::path::Path;
use std::sync::Arc;

use corvene_git::GitBinary;
use corvene_models::{Branch, Remote};

/// One change Repository Settings › Remote stages until Save.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemoteEdit {
    Add { name: String, url: String },
    Rename { from: String, to: String },
    SetUrl { name: String, url: String },
    Remove { name: String },
}

/// Where pushes go, as read when the dialog opens and as written on Save.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PushTargetConfig {
    /// `remote.pushDefault` (`None`: unset, the upstream's remote).
    pub push_default: Option<String>,
    /// The current branch and its `branch.<name>.pushRemote` (`None`: the
    /// repository's setting applies).
    pub branch: Option<(String, Option<String>)>,
}

/// The current branch's push remote while it is not the upstream's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PushTarget {
    pub remote: String,
    /// Commits of the branch that `<remote>/<branch>` lacks, or while that
    /// branch is not there yet, that no branch of the remote has.
    pub ahead: u32,
    /// `<remote>/<branch>` exists.
    pub published: bool,
}

/// A remote as the dialog edits it: its name when it opened (`None` for
/// one added here), its name and URL now, and whether it is to go.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditedRemote {
    pub original: Option<Remote>,
    pub name: String,
    pub url: String,
    pub removed: bool,
}

impl EditedRemote {
    pub fn existing(remote: &Remote) -> Self {
        Self {
            original: Some(remote.clone()),
            name: remote.name.clone(),
            url: remote.url.clone(),
            removed: false,
        }
    }
}

/// Why the edited list cannot be saved: a bad or repeated name, or an
/// empty name or URL. `None` when it can.
pub fn edited_remotes_error(remotes: &[EditedRemote]) -> Option<String> {
    let kept: Vec<&EditedRemote> = remotes.iter().filter(|r| !r.removed).collect();
    for (i, remote) in kept.iter().enumerate() {
        let name = remote.name.trim();
        if !corvene_git::remote_name_is_valid(name) {
            return Some(if name.is_empty() {
                "A remote needs a name.".to_string()
            } else {
                format!("{name} is not a valid remote name.")
            });
        }
        if remote.url.trim().is_empty() {
            return Some(format!("The remote {name} needs a URL."));
        }
        if kept[..i].iter().any(|other| other.name.trim() == name) {
            return Some(format!("There is more than one remote named {name}."));
        }
    }
    None
}

/// The edits that turn the remotes as they were into `remotes`, in the
/// order they can run: removals first (they free names), then renames,
/// URL changes and additions. A rename into a name another rename frees
/// goes through a temporary name.
pub fn remote_edits(remotes: &[EditedRemote]) -> Vec<RemoteEdit> {
    let mut removals = Vec::new();
    let mut renames: Vec<(String, String)> = Vec::new();
    let mut urls = Vec::new();
    let mut adds = Vec::new();
    for remote in remotes {
        let (name, url) = (remote.name.trim(), remote.url.trim());
        match &remote.original {
            Some(original) if remote.removed => removals.push(RemoteEdit::Remove {
                name: original.name.clone(),
            }),
            Some(original) => {
                if original.name != name {
                    renames.push((original.name.clone(), name.to_string()));
                }
                if original.url != url {
                    urls.push(RemoteEdit::SetUrl {
                        name: name.to_string(),
                        url: url.to_string(),
                    });
                }
            }
            None if remote.removed => {}
            None => adds.push(RemoteEdit::Add {
                name: name.to_string(),
                url: url.to_string(),
            }),
        }
    }
    // a rename whose target is the old name of another one waits behind a
    // temporary name (`a → b`, `b → a`): it moves aside first, the plain
    // renames run, then it takes its name
    let (mut aside, mut direct, mut parked) = (Vec::new(), Vec::new(), Vec::new());
    for (from, to) in &renames {
        if renames.iter().any(|(other, _)| other == to) {
            let temporary = format!("{to}-corvene-rename");
            aside.push(RemoteEdit::Rename {
                from: from.clone(),
                to: temporary.clone(),
            });
            parked.push(RemoteEdit::Rename {
                from: temporary,
                to: to.clone(),
            });
        } else {
            direct.push(RemoteEdit::Rename {
                from: from.clone(),
                to: to.clone(),
            });
        }
    }
    removals
        .into_iter()
        .chain(aside)
        .chain(direct)
        .chain(parked)
        .chain(urls)
        .chain(adds)
        .collect()
}

/// Runs `edits` in order; one message per failed edit (the rest still run).
pub fn apply_remote_edits(
    git: Arc<GitBinary>,
    workdir: &Path,
    edits: &[RemoteEdit],
) -> Vec<String> {
    let mut errors = Vec::new();
    for edit in edits {
        let result = match edit {
            RemoteEdit::Add { name, url } => {
                corvene_git::add_remote(git.clone(), workdir, name, url)
            }
            RemoteEdit::Rename { from, to } => {
                corvene_git::rename_remote(git.clone(), workdir, from, to)
            }
            RemoteEdit::SetUrl { name, url } => {
                corvene_git::set_remote_url(git.clone(), workdir, name, url)
            }
            RemoteEdit::Remove { name } => corvene_git::remove_remote(git.clone(), workdir, name),
        };
        if let Err(err) = result {
            errors.push(match edit {
                RemoteEdit::Add { name, .. } => format!("Could not add the remote {name}: {err}"),
                RemoteEdit::Rename { from, to } => {
                    format!("Could not rename the remote {from} to {to}: {err}")
                }
                RemoteEdit::SetUrl { name, .. } => {
                    format!("Could not change the URL of {name}: {err}")
                }
                RemoteEdit::Remove { name } => format!("Could not remove the remote {name}: {err}"),
            });
        }
    }
    errors
}

/// The push settings in effect (any config file), for the current
/// `branch`.
pub fn read_push_config(
    git: Arc<GitBinary>,
    workdir: &Path,
    branch: Option<&str>,
) -> PushTargetConfig {
    PushTargetConfig {
        push_default: corvene_git::config_value(git.clone(), workdir, "remote.pushDefault"),
        branch: branch.map(|b| {
            (
                b.to_string(),
                corvene_git::config_value(git, workdir, &format!("branch.{b}.pushRemote")),
            )
        }),
    }
}

/// Writes the settings of `after` that differ from `before` to the
/// repository's own config (`None` unsets them there).
pub fn write_push_config(
    git: Arc<GitBinary>,
    workdir: &Path,
    before: &PushTargetConfig,
    after: &PushTargetConfig,
) -> Result<(), corvene_git::GitError> {
    let set = |key: &str, value: Option<&str>| match value {
        Some(value) => corvene_git::set_local_config_value(git.clone(), workdir, key, value),
        None => corvene_git::remove_local_config_value(git.clone(), workdir, key),
    };
    if after.push_default != before.push_default {
        set("remote.pushDefault", after.push_default.as_deref())?;
    }
    if let Some((branch, remote)) = &after.branch {
        let was = before
            .branch
            .as_ref()
            .filter(|(b, _)| b == branch)
            .and_then(|(_, r)| r.as_ref());
        if remote.as_ref() != was {
            set(&format!("branch.{branch}.pushRemote"), remote.as_deref())?;
        }
    }
    Ok(())
}

/// `branch`'s push remote ([`PushTarget`]) when it is set, exists and is
/// not where the branch goes anyway: the upstream's remote, or for a branch
/// without one `default_remote`.
pub fn read_push_target(
    git: Arc<GitBinary>,
    workdir: &Path,
    branch: &Branch,
    remotes: &[Remote],
    branches: &[Branch],
    default_remote: Option<&str>,
) -> Option<PushTarget> {
    let config = read_push_config(git.clone(), workdir, Some(&branch.name));
    let name = config
        .branch
        .and_then(|(_, remote)| remote)
        .or(config.push_default)?;
    if !remotes.iter().any(|r| r.name == name) {
        return None;
    }
    let goes_to = branch.upstream_remote_name().or(default_remote);
    if goes_to == Some(name.as_str()) {
        return None;
    }
    let tracking = format!("refs/remotes/{name}/{}", branch.name);
    let published = branches.iter().any(|b| b.full_name == tracking);
    let ahead = if published {
        corvene_git::symmetric_ahead_behind(git, workdir, &branch.full_name, &tracking)
            .ok()
            .flatten()
            .map_or(0, |ab| ab.ahead)
    } else {
        corvene_git::commits_not_on_remote(git, workdir, &branch.full_name, &name).unwrap_or(0)
    };
    Some(PushTarget {
        remote: name,
        ahead,
        published,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote(name: &str) -> Remote {
        Remote {
            name: name.to_string(),
            url: format!("https://example.com/{name}.git"),
        }
    }

    #[test]
    fn orders_edits() {
        let mut origin = EditedRemote::existing(&remote("origin"));
        origin.url = "git@example.com:o.git".into();
        let mut fork = EditedRemote::existing(&remote("fork"));
        fork.name = "mine".into();
        let mut old = EditedRemote::existing(&remote("old"));
        old.removed = true;
        let added = EditedRemote {
            original: None,
            name: "upstream".into(),
            url: "https://example.com/u.git".into(),
            removed: false,
        };
        assert_eq!(
            remote_edits(&[origin, fork, old, added]),
            vec![
                RemoteEdit::Remove { name: "old".into() },
                RemoteEdit::Rename {
                    from: "fork".into(),
                    to: "mine".into()
                },
                RemoteEdit::SetUrl {
                    name: "origin".into(),
                    url: "git@example.com:o.git".into()
                },
                RemoteEdit::Add {
                    name: "upstream".into(),
                    url: "https://example.com/u.git".into()
                },
            ]
        );
    }

    #[test]
    fn swaps_names_through_a_temporary_one() {
        let mut a = EditedRemote::existing(&remote("a"));
        a.name = "b".into();
        let mut b = EditedRemote::existing(&remote("b"));
        b.name = "a".into();
        assert_eq!(
            remote_edits(&[a, b]),
            vec![
                RemoteEdit::Rename {
                    from: "a".into(),
                    to: "b-corvene-rename".into()
                },
                RemoteEdit::Rename {
                    from: "b".into(),
                    to: "a-corvene-rename".into()
                },
                RemoteEdit::Rename {
                    from: "b-corvene-rename".into(),
                    to: "b".into()
                },
                RemoteEdit::Rename {
                    from: "a-corvene-rename".into(),
                    to: "a".into()
                },
            ]
        );
    }

    #[test]
    fn rejects_bad_lists() {
        let mut dup = EditedRemote::existing(&remote("fork"));
        dup.name = "origin".into();
        assert!(edited_remotes_error(&[EditedRemote::existing(&remote("origin")), dup]).is_some());
        let mut blank = EditedRemote::existing(&remote("x"));
        blank.url = " ".into();
        assert!(edited_remotes_error(&[blank]).is_some());
        let mut removed = EditedRemote::existing(&remote("fork"));
        removed.name = "origin".into();
        removed.removed = true;
        assert!(
            edited_remotes_error(&[EditedRemote::existing(&remote("origin")), removed]).is_none()
        );
    }
}
