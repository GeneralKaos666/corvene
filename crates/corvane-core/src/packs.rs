//! On-demand packs in the app (`corvane_packs`): which packs
//! are installed, the manifest for Settings › Advanced, download / remove
//! with progress, and handing the tree-sitter grammar libraries to
//! `corvane_highlight`. No GHD equivalent (Electron
//! ships everything, and has no tree-sitter).
//!
//! Testing hooks: `CORVANE_PACKS_MANIFEST=<url|path>` (the manifest),
//! `CORVANE_INSTALL_PACK=<name>` installs that pack at launch.

use std::collections::HashMap;

use corvane_models::SyntaxHighlighter;
use corvane_packs::{InstalledPack, PackError, PackKind, PackManifest};
use gpui_kit::{App, AsyncApp};
use tracing::{error, info, warn};

use crate::dispatcher::Dispatcher;
use crate::flags::{Flags, ids};
use crate::remote::spawn_bg;

/// A download / install in flight.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackProgress {
    pub received: u64,
    pub total: Option<u64>,
}

/// `AppState::packs`
#[derive(Clone, Debug, Default)]
pub struct PacksState {
    /// The manifest, once fetched for Settings › Advanced.
    pub manifest: Option<PackManifest>,
    pub manifest_error: Option<String>,
    pub manifest_loading: bool,
    pub installed: HashMap<PackKind, InstalledPack>,
    pub progress: HashMap<PackKind, PackProgress>,
    /// The last install / remove error per pack, shown next to its row.
    pub errors: HashMap<PackKind, String>,
}

impl PacksState {
    /// Whether the pack's contents are usable: compiled in (full build) or
    /// installed.
    pub fn available(&self, kind: PackKind) -> bool {
        self.bundled(kind) || self.installed.contains_key(&kind)
    }

    /// Whether this build compiled the pack in.
    pub fn bundled(&self, kind: PackKind) -> bool {
        match kind {
            PackKind::TreeSitterAll | PackKind::TreeSitterRest => {
                corvane_highlight::treesitter::bundled()
            }
            PackKind::GitPortable | PackKind::GitLfs => false,
        }
    }

    /// The pack a Syntax highlighting choice still needs (`None` when it
    /// needs none or what it needs is here). The `tree-sitter-all` pack
    /// covers what `tree-sitter-rest` has.
    pub fn missing_for(&self, highlighter: SyntaxHighlighter) -> Option<PackKind> {
        match highlighter {
            SyntaxHighlighter::GitHubDesktop => None,
            SyntaxHighlighter::TreeSitter => {
                (!self.available(PackKind::TreeSitterAll)).then_some(PackKind::TreeSitterAll)
            }
            SyntaxHighlighter::TreeSitterFallback => (!self.available(PackKind::TreeSitterAll)
                && !self.available(PackKind::TreeSitterRest))
            .then_some(PackKind::TreeSitterRest),
        }
    }
}

/// Every pack Settings › Advanced may list (the git packs are not published
/// yet).
pub const OFFERED_PACKS: &[PackKind] = &[PackKind::TreeSitterAll, PackKind::TreeSitterRest];

/// The packs the flags offer: the tree-sitter grammars with
/// `105-tree-sitter-highlighting`.
pub fn offered_packs(flags: &Flags) -> Vec<PackKind> {
    OFFERED_PACKS
        .iter()
        .copied()
        // Google Play delivers one module with every grammar
        .filter(|kind| {
            *kind != PackKind::TreeSitterRest
                || !corvane_packs::store_delivered(PackKind::TreeSitterAll)
        })
        .filter(|kind| match kind {
            PackKind::TreeSitterAll | PackKind::TreeSitterRest => {
                flags.bool(ids::TREE_SITTER_HIGHLIGHTING)
            }
            PackKind::GitPortable | PackKind::GitLfs => false,
        })
        .collect()
}

fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Where a tree-sitter pack's grammar libraries are unpacked on first use
/// (`~/Library/Caches/Corvane/grammars/<pack>/`, one folder per version).
fn grammar_cache(kind: PackKind) -> std::path::PathBuf {
    corvane_platform::paths::cache_dir()
        .join("grammars")
        .join(kind.name())
}

/// Point the consumer at an installed pack's data.
fn activate(pack: &InstalledPack) -> Result<(), String> {
    match pack.kind {
        PackKind::TreeSitterAll | PackKind::TreeSitterRest => {
            let count = corvane_highlight::treesitter::load_pack(
                pack.kind.name(),
                &pack.entry_path(),
                &grammar_cache(pack.kind).join(&pack.version),
            )
            .map_err(|err| {
                warn!(%err, pack = pack.kind.name(), "could not load the tree-sitter grammars");
                err
            })?;
            info!(count, version = %pack.version, pack = pack.kind.name(), "tree-sitter grammars loaded");
            Ok(())
        }
        PackKind::GitPortable | PackKind::GitLfs => Ok(()),
    }
}

fn deactivate(kind: PackKind) {
    match kind {
        PackKind::TreeSitterAll | PackKind::TreeSitterRest => {
            corvane_highlight::treesitter::unload_library(kind.name());
            let _ = std::fs::remove_dir_all(grammar_cache(kind));
        }
        PackKind::GitPortable | PackKind::GitLfs => {}
    }
}

impl Dispatcher {
    /// At launch: find the installed packs the flags offer and activate them.
    pub fn load_installed_packs(cx: &mut App) {
        let offered: Vec<PackKind> = {
            let s = Self::state(cx).read(cx);
            offered_packs(&s.flags)
                .into_iter()
                .filter(|kind| !s.packs.bundled(*kind))
                .collect()
        };
        if offered.is_empty() {
            info!("no optional components offered: installed packs stay inactive");
            return;
        }
        Self::load_packs(offered, cx);
    }

    /// Activate the installed ones of `kinds` (in the background); honours
    /// `CORVANE_INSTALL_PACK` for them afterwards.
    fn load_packs(kinds: Vec<PackKind>, cx: &mut App) {
        let wanted = kinds.clone();
        spawn_bg(
            cx,
            move || {
                let mut found = Vec::new();
                for kind in wanted {
                    if let Some(pack) = corvane_packs::installed(kind) {
                        let error = activate(&pack).err();
                        found.push((pack, error));
                    }
                }
                found
            },
            move |found, cx| {
                Self::state(cx).update(cx, |s, cx| {
                    for (pack, error) in found {
                        match error {
                            Some(err) => s.packs.errors.insert(pack.kind, err),
                            None => s.packs.errors.remove(&pack.kind),
                        };
                        s.packs.installed.insert(pack.kind, pack);
                    }
                    cx.notify();
                });
                if let Ok(name) = std::env::var("CORVANE_INSTALL_PACK")
                    && let Some(kind) = kinds.iter().find(|k| k.name() == name)
                {
                    Self::install_pack(*kind, cx);
                }
            },
        );
    }

    /// `105-tree-sitter-highlighting` turned on: load the grammar packs
    /// already on disk.
    pub(crate) fn load_tree_sitter_packs(cx: &mut App) {
        let kinds: Vec<PackKind> = [PackKind::TreeSitterAll, PackKind::TreeSitterRest]
            .into_iter()
            .filter(|kind| {
                let packs = &Self::state(cx).read(cx).packs;
                !packs.bundled(*kind) && !packs.installed.contains_key(kind)
            })
            .collect();
        if !kinds.is_empty() {
            Self::load_packs(kinds, cx);
        }
    }

    /// Fetch the manifest (Settings › Advanced opening, Retry).
    pub fn refresh_packs_manifest(cx: &mut App) {
        let state = Self::state(cx);
        if state.read(cx).packs.manifest_loading {
            return;
        }
        state.update(cx, |s, cx| {
            s.packs.manifest_loading = true;
            s.packs.manifest_error = None;
            cx.notify();
        });
        spawn_bg(cx, corvane_packs::fetch_manifest, |result, cx| {
            Self::state(cx).update(cx, |s, cx| {
                s.packs.manifest_loading = false;
                match result {
                    Ok(manifest) => s.packs.manifest = Some(manifest),
                    Err(err) => {
                        warn!(%err, "could not fetch the packs manifest");
                        s.packs.manifest_error = Some(err.to_string());
                    }
                }
                cx.notify();
            });
        });
    }

    /// Download, verify and install `kind` from the manifest (fetching the
    /// manifest first when needed), then activate it.
    pub fn install_pack(kind: PackKind, cx: &mut App) {
        #[cfg(target_os = "android")]
        if matches!(kind, PackKind::TreeSitterAll | PackKind::TreeSitterRest)
            && corvane_packs::store_delivered(PackKind::TreeSitterAll)
        {
            return Self::install_play_grammars(cx);
        }
        let state = Self::state(cx);
        let entry = {
            let s = state.read(cx);
            if s.packs.progress.contains_key(&kind) {
                return;
            }
            s.packs
                .manifest
                .as_ref()
                .and_then(|m| m.entry_for(kind, app_version()).cloned())
        };
        let Some(entry) = entry else {
            // no manifest yet: fetch it, then try again
            let has_manifest = state.read(cx).packs.manifest.is_some();
            if has_manifest {
                Self::state(cx).update(cx, |s, cx| {
                    s.packs.errors.insert(
                        kind,
                        format!(
                            "{} is not available for Corvane {}",
                            kind.title(),
                            app_version()
                        ),
                    );
                    cx.notify();
                });
                return;
            }
            state.update(cx, |s, cx| {
                s.packs.manifest_loading = true;
                s.packs.manifest_error = None;
                s.packs.progress.insert(
                    kind,
                    PackProgress {
                        received: 0,
                        total: None,
                    },
                );
                cx.notify();
            });
            spawn_bg(cx, corvane_packs::fetch_manifest, move |result, cx| {
                Self::state(cx).update(cx, |s, cx| {
                    s.packs.manifest_loading = false;
                    s.packs.progress.remove(&kind);
                    match &result {
                        Ok(manifest) => s.packs.manifest = Some(manifest.clone()),
                        Err(err) => {
                            s.packs.manifest_error = Some(err.to_string());
                            s.packs.errors.insert(kind, err.to_string());
                        }
                    }
                    cx.notify();
                });
                if result.is_ok() {
                    Self::install_pack(kind, cx);
                }
            });
            return;
        };
        state.update(cx, |s, cx| {
            s.packs.errors.remove(&kind);
            s.packs.progress.insert(
                kind,
                PackProgress {
                    received: 0,
                    total: Some(entry.size).filter(|s| *s > 0),
                },
            );
            cx.notify();
        });
        let (tx, rx) = async_channel::unbounded::<(u64, Option<u64>)>();
        cx.spawn({
            let state = state.clone();
            async move |cx: &mut AsyncApp| {
                while let Ok((received, total)) = rx.recv().await {
                    state.update(cx, |s, cx| {
                        if let Some(p) = s.packs.progress.get_mut(&kind) {
                            p.received = received;
                            if total.is_some() {
                                p.total = total;
                            }
                            cx.notify();
                        }
                    });
                }
            }
        })
        .detach();
        spawn_bg(
            cx,
            move || {
                let mut last = 0u64;
                let mut progress = |received: u64, total: Option<u64>| {
                    if received == 0 || received - last >= 256 * 1024 {
                        last = received;
                        let _ = tx.try_send((received, total));
                    }
                };
                let pack = corvane_packs::install(&entry, app_version(), &mut progress)?;
                let error = activate(&pack).err();
                Ok::<(InstalledPack, Option<String>), PackError>((pack, error))
            },
            move |result, cx| {
                Self::state(cx).update(cx, |s, cx| {
                    s.packs.progress.remove(&kind);
                    match result {
                        Ok((pack, error)) => {
                            if let Some(err) = error {
                                s.packs.errors.insert(kind, err);
                            }
                            s.packs.installed.insert(kind, pack);
                        }
                        Err(err) => {
                            error!(%err, pack = kind.name(), "could not install the pack");
                            s.packs.errors.insert(kind, err.to_string());
                        }
                    }
                    cx.notify();
                });
            },
        );
    }

    /// Android's `play` flavour: the grammars are a feature module Google
    /// Play installs on demand (`CorvaneActivity` › `GrammarModule`); its
    /// progress shows like a download's, and once installed it is loaded
    /// like a pack.
    #[cfg(target_os = "android")]
    fn install_play_grammars(cx: &mut App) {
        use corvane_platform::android::GrammarModuleEvent;
        const KIND: PackKind = PackKind::TreeSitterAll;
        let state = Self::state(cx);
        if state.read(cx).packs.progress.contains_key(&KIND) {
            return;
        }
        let Some(bridge) = corvane_platform::android::bridge() else {
            return;
        };
        state.update(cx, |s, cx| {
            s.packs.errors.remove(&KIND);
            s.packs.progress.insert(
                KIND,
                PackProgress {
                    received: 0,
                    total: None,
                },
            );
            cx.notify();
        });
        // one listener for the life of the process; later installs reuse it
        thread_local! {
            static LISTENING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        }
        if !LISTENING.replace(true) {
            let (tx, rx) = async_channel::unbounded::<GrammarModuleEvent>();
            corvane_platform::android::set_grammar_module_handler(move |event| {
                let _ = tx.try_send(event);
            });
            cx.spawn(async move |cx: &mut AsyncApp| {
                while let Ok(event) = rx.recv().await {
                    cx.update(|cx| match event {
                        GrammarModuleEvent::Progress { received, total } => {
                            Self::state(cx).update(cx, |s, cx| {
                                if let Some(progress) = s.packs.progress.get_mut(&KIND) {
                                    progress.received = received;
                                    progress.total = Some(total).filter(|total| *total > 0);
                                    cx.notify();
                                }
                            });
                        }
                        GrammarModuleEvent::Installed => {
                            Self::state(cx).update(cx, |s, cx| {
                                s.packs.progress.remove(&KIND);
                                cx.notify();
                            });
                            Self::load_packs(vec![KIND], cx);
                        }
                        GrammarModuleEvent::Failed(err) => {
                            error!(%err, "Google Play could not install the grammars");
                            Self::state(cx).update(cx, |s, cx| {
                                s.packs.progress.remove(&KIND);
                                s.packs.errors.insert(KIND, err);
                                cx.notify();
                            });
                        }
                    });
                }
            })
            .detach();
        }
        bridge.install_grammar_module();
    }

    /// Remove `kind` from disk and fall back to the compiled-in data.
    pub fn uninstall_pack(kind: PackKind, cx: &mut App) {
        spawn_bg(
            cx,
            move || {
                deactivate(kind);
                #[cfg(target_os = "android")]
                if corvane_packs::store_delivered(kind) {
                    if let Some(bridge) = corvane_platform::android::bridge() {
                        bridge.uninstall_grammar_module();
                    }
                    return Ok(());
                }
                corvane_packs::uninstall(kind)
            },
            move |result, cx| {
                Self::state(cx).update(cx, |s, cx| {
                    match result {
                        Ok(()) => {
                            s.packs.installed.remove(&kind);
                            s.packs.errors.remove(&kind);
                        }
                        Err(err) => {
                            s.packs.errors.insert(kind, err.to_string());
                        }
                    }
                    cx.notify();
                });
            },
        );
    }
}
