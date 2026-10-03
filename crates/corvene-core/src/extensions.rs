//! Language extensions in the app (Corvene addition, flag
//! `111-language-extensions`; GitHub Desktop highlights a fixed set of
//! CodeMirror modes). `corvene_extensions` reads and converts the
//! extensions; this module owns what the UI sees ([`ExtensionsState`]),
//! the install / remove / enable flows, and hands the prepared grammars to
//! `corvene_highlight::user` (the syntect set, rebuilt and cached as a dump
//! when the enabled set changes) and to the tree-sitter registry.
//!
//! Testing hooks: `CORVENE_EXTENSIONS_DIR=<dir>` (where extensions live),
//! `CORVENE_INSTALL_EXTENSION=<path>` installs that file or folder at
//! launch.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use corvene_extensions::github::RepoRef;
use corvene_extensions::importer::ImportCandidate;
use corvene_extensions::install::{self, GrammarKind, Installed, Metadata, Resolution, Source, SourceKind, Status};
use corvene_extensions::manifest::GrammarRef;
use corvene_extensions::registry::{self, Candidate, Registry};
use corvene_highlight::treesitter::{self, UserGrammar, UserLanguage};
use corvene_highlight::user;
use gpui_kit::{App, AsyncApp};
use tracing::{info, warn};

use crate::dispatcher::Dispatcher;
use crate::flags::ids;
use crate::persistence::{ExtensionSwitches, LanguageExtensionsPrefs, StoreExt};
use crate::remote::spawn_bg;

/// Where installed extensions live (`~/Library/Application Support/Corvene/extensions`).
pub fn extensions_dir() -> PathBuf {
    std::env::var_os("CORVENE_EXTENSIONS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| corvene_platform::paths::app_support_dir().join("extensions"))
}

/// Downloads, staging and built grammar libraries
/// (`~/Library/Caches/Corvene/extensions`).
pub fn extensions_cache_dir() -> PathBuf {
    corvene_platform::paths::cache_dir().join("extensions")
}

/// What the Language Extensions dialog opens on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExtensionsFocus {
    /// the Find tab with this file suffix looked up
    Suffix(String),
    Find,
    Import,
}

/// An install in flight, by stage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExtensionProgress {
    Downloading { received: u64, total: Option<u64> },
    Unpacking,
    Converting,
}

impl ExtensionProgress {
    pub fn describe(&self) -> String {
        match self {
            ExtensionProgress::Downloading { received, total } => match total {
                Some(total) if *total > 0 => format!(
                    "Downloading {} of {}",
                    human_bytes(*received),
                    human_bytes(*total)
                ),
                _ => format!("Downloading {}", human_bytes(*received)),
            },
            ExtensionProgress::Unpacking => "Unpacking…".to_string(),
            ExtensionProgress::Converting => "Converting grammars…".to_string(),
        }
    }
}

pub fn human_bytes(n: u64) -> String {
    if n >= 1024 * 1024 {
        format!("{:.1} MB", n as f64 / (1024.0 * 1024.0))
    } else if n >= 1024 {
        format!("{} KB", n / 1024)
    } else {
        format!("{n} B")
    }
}

/// Where an extension to install comes from.
#[derive(Clone, Debug, PartialEq)]
pub enum InstallSource {
    /// a file (archive or grammar) or folder on this machine
    LocalPath(PathBuf),
    /// an archive or grammar file at an `https://` address
    Url(String),
    /// a GitHub repository (optionally a folder in it)
    GitHub(RepoRef),
    /// a registry's extension
    Registry(Candidate),
    /// an extension of an editor installed on this machine
    Import(ImportCandidate),
}

impl InstallSource {
    /// The progress / error key before the extension id is known.
    pub fn key(&self) -> String {
        match self {
            InstallSource::LocalPath(path) => format!("pending:{}", path.display()),
            InstallSource::Url(url) => format!("pending:{url}"),
            InstallSource::GitHub(repo) => format!("pending:{}", repo.url()),
            InstallSource::Registry(candidate) => format!("pending:{}", registry::extension_id(candidate)),
            InstallSource::Import(candidate) => format!("pending:{}", candidate.path.display()),
        }
    }

    /// What the pending row calls it.
    pub fn label(&self) -> String {
        match self {
            InstallSource::LocalPath(path) => path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string()),
            InstallSource::Url(url) => url
                .rsplit('/')
                .next()
                .filter(|n| !n.is_empty())
                .unwrap_or(url)
                .to_string(),
            InstallSource::GitHub(repo) => format!("{}/{}", repo.owner, repo.repo),
            InstallSource::Registry(candidate) => candidate.display_name.clone(),
            InstallSource::Import(candidate) => candidate.display_name.clone(),
        }
    }

    /// Whether this source is downloaded.
    pub fn is_remote(&self) -> bool {
        matches!(self, InstallSource::Url(_) | InstallSource::GitHub(_) | InstallSource::Registry(_))
    }
}

/// The Find tab.
#[derive(Clone, Debug, Default)]
pub struct SearchState {
    pub query: String,
    /// results of the latest search, sorted by registry then downloads
    pub results: Vec<Candidate>,
    pub in_flight: BTreeSet<Registry>,
    pub errors: HashMap<Registry, String>,
    /// bumped per search so late answers to an earlier one are dropped
    pub generation: u64,
    /// the search was a suffix lookup (`.foo`) rather than free text
    pub suffix: Option<String>,
}

/// What a suffix lookup (the diff hint) found.
#[derive(Clone, Debug, Default)]
pub struct HintResult {
    pub candidates: Vec<Candidate>,
    pub in_flight: bool,
    pub error: Option<String>,
}

/// `AppState::extensions`.
#[derive(Clone, Debug, Default)]
pub struct ExtensionsState {
    /// Installed extensions, sorted by display name.
    pub installed: Vec<Installed>,
    /// The folder has been read once.
    pub loaded: bool,
    pub loading: bool,
    pub prefs: LanguageExtensionsPrefs,
    /// Installs in flight: by extension id, or `pending:<source>` before
    /// the id is known.
    pub progress: HashMap<String, ExtensionProgress>,
    /// The last error per extension id (or pending key).
    pub errors: HashMap<String, String>,
    /// The user grammar set is being rebuilt.
    pub rebuilding: bool,
    /// The row shown in the details panel.
    pub selected: Option<String>,
    /// Extension ids whose tree-sitter grammars are registered.
    pub registered_tree_sitter: BTreeSet<String>,
    pub search: SearchState,
    /// Import from editors: `None` until scanned.
    pub import_candidates: Option<Vec<ImportCandidate>>,
    pub import_scanning: bool,
    /// Suffix → the registries' candidates (the diff hint, the Find tab).
    pub hint_lookup: HashMap<String, HintResult>,
    /// Extension id → the newer version a registry has.
    pub updates: HashMap<String, Candidate>,
    pub checking_updates: bool,
}

impl ExtensionsState {
    pub fn get(&self, id: &str) -> Option<&Installed> {
        self.installed.iter().find(|i| i.metadata.id == id)
    }

    /// Extensions enabled, in order.
    pub fn enabled(&self) -> impl Iterator<Item = &Installed> {
        self.installed.iter().filter(|i| i.metadata.enabled)
    }

    /// Apply the stored switches to freshly read metadata (the store wins).
    fn apply_switches(&mut self) {
        for installed in &mut self.installed {
            if let Some(switches) = self.prefs.switches.get(&installed.metadata.id) {
                installed.metadata.enabled = switches.enabled;
                installed.metadata.prefer_over_builtin = switches.prefer_over_builtin;
            }
        }
    }

    fn upsert(&mut self, installed: Installed) {
        self.installed.retain(|i| i.metadata.id != installed.metadata.id);
        self.installed.push(installed);
        self.installed.sort_by(|a, b| {
            a.metadata
                .display_name
                .to_lowercase()
                .cmp(&b.metadata.display_name.to_lowercase())
        });
    }
}

/// What a rebuild of the user layer produced.
struct Rebuilt {
    set: Option<(corvene_extensions::cache::SyntaxSet, HashMap<String, (String, bool)>)>,
    tree_sitter: Vec<(String, bool, Vec<UserGrammar>)>,
    errors: Vec<(String, String)>,
}

impl Dispatcher {
    fn extensions_enabled(cx: &App) -> bool {
        Self::state(cx).read(cx).flags.bool(ids::LANGUAGE_EXTENSIONS)
    }

    fn save_extension_prefs(cx: &mut App) {
        let (store, prefs) = {
            let s = Self::state(cx).read(cx);
            (s.store.clone(), s.extensions.prefs.clone())
        };
        if let Err(err) = store.save_language_extensions(&prefs) {
            warn!(%err, "could not save the language extension settings");
        }
    }

    /// At launch (and when `111-language-extensions` turns on): read the
    /// installed extensions and load their grammars. Honours
    /// `CORVENE_INSTALL_EXTENSION` afterwards.
    pub fn load_language_extensions(cx: &mut App) {
        if !Self::extensions_enabled(cx) {
            return;
        }
        let state = Self::state(cx);
        let prefs = state
            .read(cx)
            .store
            .language_extensions()
            .unwrap_or_default();
        state.update(cx, |s, cx| {
            s.extensions.prefs = prefs;
            s.extensions.loading = true;
            cx.notify();
        });
        let dir = extensions_dir();
        spawn_bg(
            cx,
            move || install::list(&dir),
            move |installed, cx| {
                Self::state(cx).update(cx, |s, cx| {
                    s.extensions.installed = installed;
                    s.extensions.apply_switches();
                    s.extensions.loaded = true;
                    s.extensions.loading = false;
                    cx.notify();
                });
                let count = Self::state(cx).read(cx).extensions.installed.len();
                if count > 0 {
                    info!(count, "language extensions found");
                }
                Self::rebuild_user_syntaxes(cx);
                if let Ok(path) = std::env::var("CORVENE_INSTALL_EXTENSION")
                    && !path.is_empty()
                {
                    Self::install_extension(InstallSource::LocalPath(PathBuf::from(path)), cx);
                }
            },
        );
    }

    /// `111-language-extensions` turned off: drop the user grammars from
    /// the highlighter (the folders stay).
    pub(crate) fn unload_language_extensions(cx: &mut App) {
        let registered = Self::state(cx).update(cx, |s, cx| {
            s.extensions.loaded = false;
            cx.notify();
            std::mem::take(&mut s.extensions.registered_tree_sitter)
        });
        for id in registered {
            treesitter::unregister_user(&id);
        }
        user::clear();
    }

    /// Rebuild the user grammar set from the enabled extensions and hand it
    /// to the highlighter; register their tree-sitter grammars.
    pub fn rebuild_user_syntaxes(cx: &mut App) {
        let state = Self::state(cx);
        let enabled: Vec<Installed> = state.read(cx).extensions.enabled().cloned().collect();
        let previously = state.update(cx, |s, cx| {
            s.extensions.rebuilding = true;
            cx.notify();
            s.extensions.registered_tree_sitter.clone()
        });
        let dir = extensions_dir();
        spawn_bg(
            cx,
            move || rebuild(&enabled, &dir),
            move |rebuilt, cx| {
                for id in &previously {
                    treesitter::unregister_user(id);
                }
                let mut registered = BTreeSet::new();
                for (id, preferred, grammars) in rebuilt.tree_sitter {
                    if !grammars.is_empty() {
                        treesitter::register_user(&id, preferred, grammars);
                        registered.insert(id);
                    }
                }
                match rebuilt.set {
                    Some((set, owners)) => user::install_set(set, owners),
                    None => user::clear(),
                }
                Self::state(cx).update(cx, |s, cx| {
                    s.extensions.rebuilding = false;
                    s.extensions.registered_tree_sitter = registered;
                    for (id, error) in rebuilt.errors {
                        s.extensions.errors.insert(id, error);
                    }
                    cx.notify();
                });
            },
        );
    }

    /// Install an extension from `source`: download or copy, unpack, read,
    /// convert, then rebuild the user grammars.
    pub fn install_extension(source: InstallSource, cx: &mut App) {
        let key = source.key();
        let state = Self::state(cx);
        if state.read(cx).extensions.progress.contains_key(&key) {
            return;
        }
        let first = if source.is_remote() {
            ExtensionProgress::Downloading {
                received: 0,
                total: None,
            }
        } else {
            ExtensionProgress::Unpacking
        };
        state.update(cx, |s, cx| {
            s.extensions.errors.remove(&key);
            s.extensions.progress.insert(key.clone(), first);
            cx.notify();
        });
        let (tx, rx) = async_channel::unbounded::<ExtensionProgress>();
        cx.spawn({
            let state = state.clone();
            let key = key.clone();
            async move |cx: &mut AsyncApp| {
                while let Ok(progress) = rx.recv().await {
                    state.update(cx, |s, cx| {
                        if let Some(slot) = s.extensions.progress.get_mut(&key) {
                            *slot = progress;
                            cx.notify();
                        }
                    });
                }
            }
        })
        .detach();
        let dir = extensions_dir();
        let staging = extensions_cache_dir()
            .join("downloads")
            .join(format!("staging-{}", install::now()));
        let for_bg = source.clone();
        spawn_bg(
            cx,
            move || {
                let mut report = |progress: ExtensionProgress| {
                    let _ = tx.try_send(progress);
                };
                let result = install_from(&for_bg, &dir, &staging, &mut report);
                let _ = std::fs::remove_dir_all(&staging);
                result
            },
            move |result, cx| {
                Self::state(cx).update(cx, |s, cx| {
                    s.extensions.progress.remove(&key);
                    match &result {
                        Ok(installed) => {
                            let id = installed.metadata.id.clone();
                            s.extensions.errors.remove(&id);
                            s.extensions.updates.remove(&id);
                            s.extensions.upsert(installed.clone());
                            // a reinstall keeps the user's switches
                            s.extensions.apply_switches();
                            s.extensions.selected = Some(id);
                        }
                        Err(err) => {
                            warn!(%err, "could not install the language extension");
                            s.extensions.errors.insert(key.clone(), err.to_string());
                        }
                    }
                    cx.notify();
                });
                if result.is_ok() {
                    Self::rebuild_user_syntaxes(cx);
                }
            },
        );
    }

    /// Search the registries (the Find tab). Each registry answers on its
    /// own; a later search drops the earlier one's late answers.
    pub fn search_extensions(query: String, cx: &mut App) {
        let query = query.trim().to_string();
        let suffix = query
            .strip_prefix('.')
            .filter(|s| !s.is_empty() && !s.contains(char::is_whitespace))
            .map(str::to_ascii_lowercase);
        let generation = Self::state(cx).update(cx, |s, cx| {
            s.extensions.search.generation += 1;
            s.extensions.search.query = query.clone();
            s.extensions.search.suffix = suffix.clone();
            s.extensions.search.results.clear();
            s.extensions.search.errors.clear();
            s.extensions.search.in_flight = if query.is_empty() {
                BTreeSet::new()
            } else {
                Registry::ALL.into_iter().collect()
            };
            cx.notify();
            s.extensions.search.generation
        });
        if query.is_empty() {
            return;
        }
        for registry in Registry::ALL {
            let (query, suffix) = (query.clone(), suffix.clone());
            spawn_bg(
                cx,
                move || match &suffix {
                    Some(suffix) => registry.for_suffix(suffix),
                    None => registry.search(&query),
                },
                move |result, cx| {
                    Self::state(cx).update(cx, |s, cx| {
                        let search = &mut s.extensions.search;
                        if search.generation != generation {
                            return;
                        }
                        search.in_flight.remove(&registry);
                        match result {
                            Ok(found) => {
                                search.results.extend(found);
                                search.results.sort_by(|a, b| {
                                    a.registry
                                        .cmp(&b.registry)
                                        .then(b.downloads.cmp(&a.downloads))
                                        .then(a.display_name.cmp(&b.display_name))
                                });
                            }
                            Err(err) => {
                                search.errors.insert(registry, err.to_string());
                            }
                        }
                        cx.notify();
                    });
                },
            );
        }
    }

    /// The registries' extensions for files with `suffix` (the diff hint).
    /// Zed's suggestion table answers at once; the others are asked.
    pub fn lookup_extensions_for_suffix(suffix: &str, cx: &mut App) {
        let suffix = suffix.trim_start_matches('.').to_ascii_lowercase();
        let state = Self::state(cx);
        if state
            .read(cx)
            .extensions
            .hint_lookup
            .get(&suffix)
            .is_some_and(|h| h.in_flight || !h.candidates.is_empty())
        {
            return;
        }
        state.update(cx, |s, cx| {
            s.extensions.hint_lookup.insert(
                suffix.clone(),
                HintResult {
                    in_flight: true,
                    ..Default::default()
                },
            );
            cx.notify();
        });
        let wanted = suffix.clone();
        spawn_bg(
            cx,
            move || {
                let mut candidates = Vec::new();
                let mut errors = Vec::new();
                for registry in Registry::ALL {
                    match registry.for_suffix(&wanted) {
                        Ok(found) => candidates.extend(found),
                        Err(err) => errors.push(format!("{}: {err}", registry.title())),
                    }
                }
                candidates.sort_by(|a, b| b.downloads.cmp(&a.downloads));
                (candidates, errors)
            },
            move |(candidates, errors), cx| {
                Self::state(cx).update(cx, |s, cx| {
                    s.extensions.hint_lookup.insert(
                        suffix,
                        HintResult {
                            candidates,
                            in_flight: false,
                            error: (!errors.is_empty()).then(|| errors.join("; ")),
                        },
                    );
                    cx.notify();
                });
            },
        );
    }

    /// Scan the editors installed on this machine for grammar extensions
    /// (the Import tab).
    pub fn scan_installed_editors(cx: &mut App) {
        let state = Self::state(cx);
        if state.read(cx).extensions.import_scanning {
            return;
        }
        state.update(cx, |s, cx| {
            s.extensions.import_scanning = true;
            cx.notify();
        });
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/"));
        spawn_bg(
            cx,
            move || corvene_extensions::importer::scan(&home),
            |found, cx| {
                Self::state(cx).update(cx, |s, cx| {
                    s.extensions.import_scanning = false;
                    s.extensions.import_candidates = Some(found);
                    cx.notify();
                });
            },
        );
    }

    /// Ask each registry-installed extension's registry for a newer version.
    pub fn check_extension_updates(cx: &mut App) {
        let state = Self::state(cx);
        if state.read(cx).extensions.checking_updates {
            return;
        }
        let targets: Vec<(String, Registry, String, Option<String>)> = state
            .read(cx)
            .extensions
            .installed
            .iter()
            .filter_map(|i| {
                let md = &i.metadata;
                let registry = match md.source.kind {
                    SourceKind::OpenVsx => Registry::OpenVsx,
                    SourceKind::Zed => Registry::Zed,
                    SourceKind::Pulsar => Registry::Pulsar,
                    _ => return None,
                };
                let registry_id = md.source.registry_id.clone()?;
                Some((md.id.clone(), registry, registry_id, md.version.clone()))
            })
            .collect();
        if targets.is_empty() {
            return;
        }
        state.update(cx, |s, cx| {
            s.extensions.checking_updates = true;
            cx.notify();
        });
        spawn_bg(
            cx,
            move || {
                let mut newer = Vec::new();
                for (id, registry, registry_id, version) in targets {
                    let latest = match registry {
                        Registry::OpenVsx => registry::openvsx::latest(&registry_id),
                        Registry::Zed => registry::zed::latest(&registry_id),
                        Registry::Pulsar => registry::pulsar::latest(&registry_id),
                    };
                    match latest {
                        Ok(Some(candidate)) if candidate.version.is_some() && candidate.version != version => {
                            newer.push((id, candidate));
                        }
                        Ok(_) => {}
                        Err(err) => warn!(%err, id, "could not check the extension for updates"),
                    }
                }
                newer
            },
            |newer, cx| {
                Self::state(cx).update(cx, |s, cx| {
                    s.extensions.checking_updates = false;
                    for (id, candidate) in newer {
                        s.extensions.updates.insert(id, candidate);
                    }
                    cx.notify();
                });
            },
        );
    }

    /// Turn an extension on or off.
    pub fn set_extension_enabled(id: &str, enabled: bool, cx: &mut App) {
        Self::update_switches(id, cx, |switches| switches.enabled = enabled);
        Self::rebuild_user_syntaxes(cx);
    }

    /// Whether an extension's grammars win over the built-in highlighting
    /// for the files they claim.
    pub fn set_extension_preferred(id: &str, preferred: bool, cx: &mut App) {
        Self::update_switches(id, cx, |switches| switches.prefer_over_builtin = preferred);
        user::set_owner_preference(id, preferred);
        if Self::state(cx).read(cx).extensions.registered_tree_sitter.contains(id) {
            // re-rank: the registration carries the preference
            Self::rebuild_user_syntaxes(cx);
        }
    }

    fn update_switches(id: &str, cx: &mut App, change: impl FnOnce(&mut ExtensionSwitches)) {
        let changed = Self::state(cx).update(cx, |s, cx| {
            let Some(installed) = s.extensions.installed.iter_mut().find(|i| i.metadata.id == id) else {
                return None;
            };
            let mut switches = ExtensionSwitches {
                enabled: installed.metadata.enabled,
                prefer_over_builtin: installed.metadata.prefer_over_builtin,
            };
            change(&mut switches);
            installed.metadata.enabled = switches.enabled;
            installed.metadata.prefer_over_builtin = switches.prefer_over_builtin;
            s.extensions.prefs.switches.insert(id.to_string(), switches);
            cx.notify();
            Some((installed.dir.clone(), installed.metadata.clone()))
        });
        if let Some((dir, metadata)) = changed {
            Self::save_extension_prefs(cx);
            spawn_bg(
                cx,
                move || install::write(&dir, &metadata),
                |result, _| {
                    if let Err(err) = result {
                        warn!(%err, "could not update the extension's metadata");
                    }
                },
            );
        }
    }

    /// Delete an extension.
    pub fn remove_extension(id: &str, cx: &mut App) {
        let removed = Self::state(cx).update(cx, |s, cx| {
            let Some(index) = s.extensions.installed.iter().position(|i| i.metadata.id == id) else {
                return None;
            };
            let installed = s.extensions.installed.remove(index);
            s.extensions.prefs.switches.remove(id);
            s.extensions.errors.remove(id);
            s.extensions.registered_tree_sitter.remove(id);
            if s.extensions.selected.as_deref() == Some(id) {
                s.extensions.selected = None;
            }
            cx.notify();
            Some(installed)
        });
        let Some(installed) = removed else {
            return;
        };
        treesitter::unregister_user(id);
        Self::save_extension_prefs(cx);
        let cache = extensions_cache_dir().join("grammars").join(id);
        spawn_bg(
            cx,
            move || {
                let result = install::remove(&installed.dir);
                let _ = std::fs::remove_dir_all(cache);
                result
            },
            |result, cx| {
                if let Err(err) = result {
                    warn!(%err, "could not remove the extension's folder");
                }
                Self::rebuild_user_syntaxes(cx);
            },
        );
    }

    /// Open the Language Extensions dialog. `return_to` reopens Settings on
    /// that tab when it closes (the dialog replaces Settings: one popup at a
    /// time).
    pub fn open_language_extensions(
        focus: Option<ExtensionsFocus>,
        return_to: Option<crate::PreferencesTab>,
        cx: &mut App,
    ) {
        if !Self::state(cx).read(cx).extensions.loaded {
            Self::load_language_extensions(cx);
        }
        Self::show_popup(crate::Popup::LanguageExtensions { focus, return_to }, cx);
    }

    /// Close the Language Extensions dialog, back to Settings when it was
    /// opened from there.
    pub fn close_language_extensions(cx: &mut App) {
        let return_to = match &Self::state(cx).read(cx).popup {
            Some(crate::Popup::LanguageExtensions { return_to, .. }) => *return_to,
            _ => None,
        };
        match return_to {
            Some(tab) => Self::show_popup(crate::Popup::Preferences { tab }, cx),
            None => Self::close_popup(cx),
        }
    }

    /// The details panel's row.
    pub fn select_extension(id: Option<String>, cx: &mut App) {
        Self::state(cx).update(cx, |s, cx| {
            if s.extensions.selected != id {
                s.extensions.selected = id;
                cx.notify();
            }
        });
    }

    /// `756-missing-highlighting-hint`: never show the hint for this
    /// suffix again.
    pub fn dismiss_suffix_hint(suffix: &str, cx: &mut App) {
        Self::state(cx).update(cx, |s, cx| {
            s.extensions
                .prefs
                .dismissed_suffixes
                .insert(suffix.to_ascii_lowercase());
            cx.notify();
        });
        Self::save_extension_prefs(cx);
    }
}

/// Where a tree-sitter grammar's parser comes from: one of Corvene's own
/// when the repository or name matches, else a build from source.
pub fn resolve_grammar(grammar: &GrammarRef) -> Resolution {
    match grammar {
        GrammarRef::TreeSitter {
            name,
            repository,
            path,
            ..
        } => match treesitter::resolve_bundled(name, repository.as_deref(), path.as_deref()) {
            Some(bundled) => Resolution::Bundled { name: bundled },
            None if repository.is_some() => Resolution::NeedsBuild,
            None => Resolution::Failed {
                error: format!("Corvene has no grammar named {name} and the extension names no repository to build it from"),
            },
        },
        _ => Resolution::Unresolved,
    }
}

/// The background half of [`Dispatcher::install_extension`].
fn install_from(
    source: &InstallSource,
    extensions_dir: &Path,
    staging: &Path,
    report: &mut dyn FnMut(ExtensionProgress),
) -> Result<Installed, corvene_extensions::ExtensionError> {
    use corvene_extensions::{http, scan};
    std::fs::create_dir_all(staging)?;
    let limits = corvene_extensions::archive::Limits::default();
    let mut download = |url: &str, name: &str, extra_host: Option<&str>| {
        let dest = staging.join(name);
        let mut progress = |received, total| {
            report(ExtensionProgress::Downloading { received, total });
        };
        let downloaded = http::download(url, &dest, http::MAX_ARCHIVE_BYTES, extra_host, &mut progress)?;
        report(ExtensionProgress::Unpacking);
        Ok::<_, corvene_extensions::ExtensionError>((dest, downloaded))
    };
    let (scanned, record, id) = match source {
        InstallSource::LocalPath(path) => {
            let scanned = scan::scan(path, staging, &limits)?;
            let kind = if path.is_dir() {
                SourceKind::LocalFolder
            } else {
                SourceKind::LocalFile
            };
            let record = Source {
                kind,
                url: None,
                path: Some(path.to_string_lossy().into_owned()),
                sha256: None,
                editor: None,
                registry_id: None,
            };
            let id = install::extension_id(kind, scanned.manifest.publisher.as_deref(), &scanned.manifest.name);
            (scanned, record, id)
        }
        InstallSource::Url(url) => {
            let name = url
                .rsplit('/')
                .next()
                .filter(|n| !n.is_empty() && !n.contains('?'))
                .unwrap_or("download.bin");
            let (file, downloaded) = download(url, name, http::host_of(url).as_deref())?;
            let scanned = scan::scan(&file, staging, &limits)?;
            let record = Source {
                kind: SourceKind::Url,
                url: Some(url.clone()),
                path: None,
                sha256: Some(downloaded.sha256),
                editor: None,
                registry_id: None,
            };
            let id = install::extension_id(SourceKind::Url, scanned.manifest.publisher.as_deref(), &scanned.manifest.name);
            (scanned, record, id)
        }
        InstallSource::GitHub(repo) => {
            let (file, downloaded) = download(&repo.tarball_url(), "repository.tar.gz", None)?;
            let unpacked = staging.join("unpacked");
            corvene_extensions::archive::extract(&file, &unpacked, &limits)?;
            // codeload wraps the tree in `<repo>-<ref>/`
            let top = std::fs::read_dir(&unpacked)?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .find(|p| p.is_dir())
                .unwrap_or(unpacked);
            let root = match &repo.path {
                Some(path) => top.join(path),
                None => top,
            };
            if !root.is_dir() {
                return Err(corvene_extensions::ExtensionError::NotAnExtension(format!(
                    "{} has no folder {}",
                    repo.url(),
                    repo.path.as_deref().unwrap_or("")
                )));
            }
            let mut scanned = scan::scan_dir(&root)?;
            if scanned.root == root && scanned.manifest.name == root.file_name().and_then(|n| n.to_str()).unwrap_or("") {
                scanned.manifest.name = repo.id_parts().1;
            }
            if scanned.manifest.repository.is_none() {
                scanned.manifest.repository = Some(repo.url());
            }
            let record = Source {
                kind: SourceKind::GitHub,
                url: Some(repo.url()),
                path: repo.path.clone(),
                sha256: Some(downloaded.sha256),
                editor: None,
                registry_id: Some(format!("{}/{}@{}", repo.owner, repo.repo, repo.reference)),
            };
            let (owner, name) = repo.id_parts();
            let id = install::extension_id(SourceKind::GitHub, Some(&owner), &name);
            (scanned, record, id)
        }
        InstallSource::Registry(candidate) => {
            let (file, downloaded) = download(&candidate.download_url, "extension.archive", None)?;
            let mut scanned = scan::scan(&file, staging, &limits)?;
            if scanned.manifest.version.is_none() {
                scanned.manifest.version = candidate.version.clone();
            }
            if scanned.manifest.repository.is_none() {
                scanned.manifest.repository = candidate.repository.clone();
            }
            if scanned.manifest.display_name.is_none() {
                scanned.manifest.display_name = Some(candidate.display_name.clone());
            }
            let record = Source {
                kind: candidate.registry.source_kind(),
                url: Some(downloaded.url),
                path: None,
                sha256: Some(downloaded.sha256),
                editor: None,
                registry_id: Some(candidate.id.clone()),
            };
            (scanned, record, registry::extension_id(candidate))
        }
        InstallSource::Import(candidate) => {
            let scanned = scan::scan(&candidate.path, staging, &limits)?;
            let record = Source {
                kind: SourceKind::Imported,
                url: None,
                path: Some(candidate.path.to_string_lossy().into_owned()),
                sha256: None,
                editor: Some(candidate.editor.title().to_string()),
                registry_id: None,
            };
            let editor = install::slug(candidate.editor.title());
            let id = install::extension_id(SourceKind::Imported, Some(&editor), &scanned.manifest.name);
            (scanned, record, id)
        }
    };
    report(ExtensionProgress::Converting);
    let out = extensions_dir.join(&id);
    let prepared = install::prepare::prepare(&scanned, &id, record, &out, &resolve_grammar)?;
    info!(id, grammars = prepared.metadata.grammars.len(), "language extension installed");
    Ok(Installed {
        dir: out,
        metadata: prepared.metadata,
    })
}

/// The background half of [`Dispatcher::rebuild_user_syntaxes`]: compile
/// the enabled extensions' sublime-syntax files into one set (from the
/// cached dump when its key matches), and collect their tree-sitter
/// grammars.
fn rebuild(enabled: &[Installed], extensions_dir: &Path) -> Rebuilt {
    use corvene_extensions::cache;
    use corvene_extensions::tm::compile::compile;
    let mut errors = Vec::new();
    let mut key_parts: Vec<String> = Vec::new();
    let mut sources: Vec<(String, bool, PathBuf, String)> = Vec::new(); // id, preferred, file, grammar name
    let mut tree_sitter = Vec::new();
    for installed in enabled {
        let md = &installed.metadata;
        let mut grammars = Vec::new();
        for grammar in &md.grammars {
            match grammar.kind {
                GrammarKind::TextMate | GrammarKind::Sublime => {
                    if grammar.status == Status::Rejected {
                        continue;
                    }
                    let Some(file) = &grammar.file else {
                        continue;
                    };
                    let path = installed.dir.join(file);
                    if let Ok(meta) = std::fs::metadata(&path) {
                        key_parts.push(format!(
                            "{}@{}:{}:{}:{}",
                            md.id,
                            md.version.as_deref().unwrap_or(""),
                            file,
                            meta.len(),
                            meta.modified()
                                .ok()
                                .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
                                .map(|d| d.as_secs())
                                .unwrap_or(0)
                        ));
                        sources.push((md.id.clone(), md.prefer_over_builtin, path, grammar.name.clone()));
                    }
                }
                GrammarKind::TreeSitter => {
                    let language = match &grammar.resolution {
                        Resolution::Bundled { name } => UserLanguage::Derived { base: name.clone() },
                        Resolution::Built { library } => UserLanguage::Library(PathBuf::from(library)),
                        Resolution::Unresolved | Resolution::NeedsBuild | Resolution::Failed { .. } => continue,
                    };
                    let Some(queries) = &grammar.queries else {
                        continue;
                    };
                    let read = |file: &str| std::fs::read_to_string(installed.dir.join(queries).join(file)).unwrap_or_default();
                    let languages: Vec<_> = md
                        .languages
                        .iter()
                        .filter(|l| l.grammar.as_deref() == Some(grammar.name.as_str()))
                        .collect();
                    let first_line = languages
                        .iter()
                        .find_map(|l| l.first_line.as_deref())
                        .and_then(|re| regex::Regex::new(re).ok());
                    grammars.push(UserGrammar {
                        name: format!("{}/{}", md.id, grammar.name),
                        extensions: languages.iter().flat_map(|l| l.suffixes.clone()).collect(),
                        filenames: languages.iter().flat_map(|l| l.filenames.clone()).collect(),
                        first_line,
                        aliases: languages.iter().flat_map(|l| l.aliases.clone()).collect(),
                        highlights: read("highlights.scm"),
                        injections: read("injections.scm"),
                        locals: read("locals.scm"),
                        language,
                    });
                }
            }
        }
        if !grammars.is_empty() {
            tree_sitter.push((md.id.clone(), md.prefer_over_builtin, grammars));
        }
    }
    if sources.is_empty() {
        for stale in dumps(extensions_dir) {
            let _ = std::fs::remove_file(stale);
        }
        return Rebuilt {
            set: None,
            tree_sitter,
            errors,
        };
    }
    let key = cache::cache_key(&key_parts);
    let dump_path = extensions_dir.join(format!("syntaxes-{key}.packdump"));
    let mut owners: HashMap<String, (String, bool)> = HashMap::new();
    let mut definitions = Vec::new();
    for (id, preferred, path, grammar_name) in &sources {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) => {
                errors.push((id.clone(), format!("{grammar_name}: {err}")));
                continue;
            }
        };
        match compile(&text, grammar_name) {
            Ok(definition) => {
                owners.insert(definition.name.clone(), (id.clone(), *preferred));
                definitions.push(definition);
            }
            Err(err) => errors.push((id.clone(), format!("{grammar_name}: {err}"))),
        }
    }
    if definitions.is_empty() {
        return Rebuilt {
            set: None,
            tree_sitter,
            errors,
        };
    }
    let set = match cache::load(&dump_path) {
        Ok(set) if dump_path.is_file() => set,
        _ => {
            let external = cache::external_scopes(&definitions);
            let base = (!external.is_empty()).then(|| {
                info!(scopes = ?external, "user grammars include built-in scopes: building on the bundled set");
                (*corvene_highlight::syntaxes::current()).clone()
            });
            let set = cache::build_set(definitions, base);
            for stale in dumps(extensions_dir) {
                let _ = std::fs::remove_file(stale);
            }
            if let Err(err) = cache::dump(&set, &dump_path) {
                warn!(%err, "could not cache the user grammar set");
            }
            set
        }
    };
    Rebuilt {
        set: Some((set, owners)),
        tree_sitter,
        errors,
    }
}

/// Every `syntaxes-*.packdump` in the folder.
fn dumps(extensions_dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(extensions_dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with("syntaxes-") && n.ends_with(".packdump"))
                })
                .collect()
        })
        .unwrap_or_default()
}

#[allow(dead_code)]
fn _metadata_type_check(_: &Metadata) {}
