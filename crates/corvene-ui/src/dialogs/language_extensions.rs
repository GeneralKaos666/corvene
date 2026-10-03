//! Language Extensions (Corvene addition, no GitHub Desktop counterpart;
//! flag `111-language-extensions`): the grammars the user installed from
//! editor extensions (`corvene_core::extensions`). Opened from Settings ›
//! Appearance › Language extensions… (and `CORVENE_POPUP=language-extensions`).
//!
//! Left: the installed extensions, one row each with its source, the file
//! types it covers, its state, and the Enabled / Prefer switches. Right: the
//! selected extension's details, including what its grammars lost in
//! conversion. Footer: Add File or Folder… (an archive, a grammar file or an
//! unpacked extension) and Close. Snapshot-verified only: the parity harness
//! has nothing to compare it with.

use std::path::PathBuf;

use corvene_core::extensions::{ExtensionProgress, ExtensionsFocus, ExtensionsState, InstallSource};
use corvene_core::{AppState, Dispatcher};
use corvene_extensions::install::{GrammarKind, Installed, Resolution, Status};
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::dialog::{DialogFrame, dialog_with_frame};
use crate::icons::{Octicon, octicon};
use crate::scrollbar::ScrollbarExt;
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{
    button, checkbox, filter_text_box, link_button, pill, primary_button, settings_description,
    switch,
};

/// The dialog's width (CSS px).
const WIDTH: f32 = 760.;
/// The list and details area's height.
const HEIGHT: f32 = 440.;
/// The details column's width.
const DETAILS_WIDTH: f32 = 280.;

pub struct LanguageExtensionsDialog {
    state: Entity<AppState>,
    filter: Entity<InputState>,
    #[allow(dead_code)]
    focus: Option<ExtensionsFocus>,
}

impl LanguageExtensionsDialog {
    pub fn new(
        state: Entity<AppState>,
        focus: Option<ExtensionsFocus>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter installed"));
        cx.observe(&filter, |_, _, cx| cx.notify()).detach();
        Self {
            state,
            filter,
            focus,
        }
    }

    fn choose(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: true,
            multiple: true,
            prompt: Some("Add".into()),
        });
        cx.spawn_in(window, async move |_, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await {
                cx.update(|_, cx| {
                    for path in paths {
                        Dispatcher::install_extension(InstallSource::LocalPath(path), cx);
                    }
                })
                .ok();
            }
        })
        .detach();
    }

    /// The installed extensions matching the filter box.
    fn visible(&self, extensions: &ExtensionsState, cx: &App) -> Vec<Installed> {
        let needle = self.filter.read(cx).value().trim().to_lowercase();
        extensions
            .installed
            .iter()
            .filter(|i| {
                needle.is_empty()
                    || i.metadata.display_name.to_lowercase().contains(&needle)
                    || i.metadata.suffixes().iter().any(|s| s.contains(&needle))
                    || i.metadata.languages.iter().any(|l| {
                        l.name
                            .as_deref()
                            .is_some_and(|n| n.to_lowercase().contains(&needle))
                    })
            })
            .cloned()
            .collect()
    }

    fn row(&self, installed: &Installed, extensions: &ExtensionsState, cx: &Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let md = &installed.metadata;
        let id = md.id.clone();
        let selected = extensions.selected.as_deref() == Some(md.id.as_str());
        let (dot, state_text) = extension_state(installed, extensions);
        let suffixes: Vec<String> = md.suffixes().iter().map(|s| format!(".{s}")).collect();
        let mut file_types = suffixes.join(" ");
        let filenames: Vec<&str> = md
            .languages
            .iter()
            .flat_map(|l| l.filenames.iter().map(String::as_str))
            .collect();
        if !filenames.is_empty() {
            if !file_types.is_empty() {
                file_types.push_str("  ");
            }
            file_types.push_str(&filenames.join(" "));
        }
        if file_types.is_empty() {
            file_types = "no file types".to_string();
        }
        let kind = grammar_kind_label(installed);
        let error = extensions.errors.get(&md.id).cloned();
        let row_id = SharedString::from(format!("lang-ext-row-{}", md.id));
        let select_id = id.clone();
        let enabled_id = id.clone();
        let prefer_id = id.clone();
        let prefer_now = md.prefer_over_builtin;
        let remove_id = id.clone();
        let (bg, hover) = if selected {
            (t.box_selected_background, t.box_selected_background)
        } else {
            (t.background, t.list_item_hover_background)
        };
        div()
            .id(row_id)
            .flex()
            .flex_col()
            .px(SPACING())
            .py(SPACING_HALF())
            .gap(zpx(3.))
            .bg(bg)
            .hover(move |s| s.bg(hover))
            .border_b_1()
            .border_color(t.box_border)
            .cursor_pointer()
            .on_click(move |_, _, cx| Dispatcher::select_extension(Some(select_id.clone()), cx))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF())
                    .child(div().size(zpx(8.)).rounded_full().flex_none().bg(dot))
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .min_w_0()
                            .truncate()
                            .child(md.display_name.clone()),
                    )
                    .child(pill(
                        md.source.kind.title(),
                        None,
                        t.list_item_badge_background,
                        t.list_item_badge_text,
                        cx,
                    ))
                    .children(md.version.clone().map(|v| {
                        div()
                            .text_size(FONT_SIZE_SM())
                            .text_color(t.text_secondary)
                            .child(v)
                    })),
            )
            .child(
                div()
                    .text_size(FONT_SIZE_SM())
                    .text_color(t.text_secondary)
                    .min_w_0()
                    .truncate()
                    .child(format!("{file_types}  ·  {kind}  ·  {state_text}")),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING())
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(SPACING_HALF())
                            .child(switch(
                                SharedString::from(format!("lang-ext-enabled-{}", md.id)),
                                md.enabled,
                                false,
                                move |on, _, cx| Dispatcher::set_extension_enabled(&enabled_id, on, cx),
                                cx,
                            ))
                            .child(div().text_size(FONT_SIZE_SM()).child("Enabled")),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("lang-ext-prefer-row-{}", md.id)))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(SPACING_HALF())
                            .cursor_pointer()
                            .child(checkbox(
                                SharedString::from(format!("lang-ext-prefer-{}", md.id)),
                                md.prefer_over_builtin,
                                false,
                                cx,
                            ))
                            .child(
                                div()
                                    .text_size(FONT_SIZE_SM())
                                    .child("Prefer over built-in highlighting"),
                            )
                            .on_click(move |_, _, cx| {
                                Dispatcher::set_extension_preferred(&prefer_id, !prefer_now, cx)
                            }),
                    )
                    .child(div().flex_1())
                    .child(
                        link_button(SharedString::from(format!("lang-ext-remove-{}", md.id)), "Remove", cx)
                            .text_size(FONT_SIZE_SM())
                            .on_click(move |_, _, cx| Dispatcher::remove_extension(&remove_id, cx)),
                    ),
            )
            .children(error.map(|e| {
                div()
                    .text_size(FONT_SIZE_SM())
                    .text_color(t.color_deleted)
                    .child(e)
            }))
            .into_any_element()
    }

    fn pending_row(&self, key: &str, progress: &ExtensionProgress, cx: &Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let what = key
            .strip_prefix("pending:")
            .and_then(|p| std::path::Path::new(p).file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "extension".to_string());
        div()
            .flex()
            .flex_col()
            .px(SPACING())
            .py(SPACING_HALF())
            .gap(zpx(3.))
            .border_b_1()
            .border_color(t.box_border)
            .child(div().font_weight(FontWeight::SEMIBOLD).child(format!("Installing {what}…")))
            .child(
                div()
                    .text_size(FONT_SIZE_SM())
                    .text_color(t.text_secondary)
                    .child(progress.describe()),
            )
            .into_any_element()
    }

    fn details(&self, extensions: &ExtensionsState, cx: &Context<Self>) -> AnyElement {
        let t = cx.ghd();
        let Some(installed) = extensions
            .selected
            .as_deref()
            .and_then(|id| extensions.get(id))
        else {
            let text = if extensions.installed.is_empty() {
                "Add an extension to highlight more languages: a .vsix or .zip from VS Code or \
                 Open VSX, a Zed or Pulsar extension, a Sublime Text package, a TextMate bundle \
                 or a single grammar file."
            } else {
                "Select an extension to see what it provides."
            };
            return div()
                .p(SPACING())
                .text_size(FONT_SIZE_SM())
                .text_color(t.text_secondary)
                .child(text)
                .into_any_element();
        };
        let md = &installed.metadata;
        let heading = |text: &str| {
            div()
                .mt(SPACING())
                .text_size(FONT_SIZE_SM())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t.text_secondary)
                .child(text.to_string())
        };
        let line = |text: String| {
            div()
                .text_size(FONT_SIZE_SM())
                .min_w_0()
                .whitespace_normal()
                .child(text)
        };
        let source = match (&md.source.url, &md.source.path, &md.source.editor) {
            (Some(url), _, _) => format!("{} · {url}", md.source.kind.title()),
            (_, Some(path), Some(editor)) => format!("{editor} · {path}"),
            (_, Some(path), None) => format!("{} · {path}", md.source.kind.title()),
            _ => md.source.kind.title().to_string(),
        };
        let dir = installed.dir.clone();
        let repository = md.repository.clone();
        let mut panel = div()
            .flex()
            .flex_col()
            .w(zpx(DETAILS_WIDTH))
            .min_w_0()
            .overflow_hidden()
            .p(SPACING())
            .gap(zpx(2.))
            .child(div().font_weight(FontWeight::SEMIBOLD).child(md.display_name.clone()))
            .children(md.description.clone().map(|d| {
                div()
                    .text_size(FONT_SIZE_SM())
                    .text_color(t.text_secondary)
                    .child(d)
            }))
            .child(heading("Source"))
            .child(
                div()
                    .text_size(FONT_SIZE_SM())
                    .text_color(t.text_secondary)
                    .min_w_0()
                    .truncate()
                    .child(source),
            )
            .children(md.version.clone().map(|v| line(format!("Version {v}"))))
            .children(md.publisher.clone().map(|p| line(format!("By {p}"))))
            .children(md.license.clone().map(|l| line(format!("License {l}"))))
            .children(repository.map(|url| {
                let shown = url.trim_start_matches("https://").to_string();
                link_button("lang-ext-repository", shown, cx)
                    .text_size(FONT_SIZE_SM())
                    .min_w_0()
                    .truncate()
                    .on_click(move |_, _, cx| Dispatcher::open_url(&url, cx))
            }))
            .child(heading("Languages"));
        if md.languages.is_empty() {
            panel = panel.child(line("None declared.".to_string()).text_color(t.text_secondary));
        }
        for language in &md.languages {
            let mut types: Vec<String> = language.suffixes.iter().map(|s| format!(".{s}")).collect();
            types.extend(language.filenames.iter().cloned());
            let name = language.name.clone().unwrap_or_else(|| language.id.clone());
            panel = panel.child(line(if types.is_empty() {
                name
            } else {
                format!("{name}: {}", types.join(", "))
            }));
        }
        panel = panel.child(heading("Grammars"));
        for grammar in &md.grammars {
            let kind = match grammar.kind {
                GrammarKind::TextMate => "TextMate",
                GrammarKind::Sublime => "Sublime",
                GrammarKind::TreeSitter => "tree-sitter",
            };
            let status = match (grammar.kind, grammar.status, &grammar.resolution) {
                (GrammarKind::TreeSitter, _, Resolution::Bundled { name }) => {
                    format!("uses Corvene's {name} grammar")
                }
                (GrammarKind::TreeSitter, _, Resolution::Built { .. }) => "built from source".to_string(),
                (GrammarKind::TreeSitter, _, Resolution::NeedsBuild) => {
                    "parser not available: needs a build from source".to_string()
                }
                (GrammarKind::TreeSitter, _, Resolution::Failed { error }) => error.clone(),
                (GrammarKind::TreeSitter, _, Resolution::Unresolved) => "not resolved".to_string(),
                (_, Status::Ok, _) if grammar.dropped > 0 => {
                    format!("ok, {} rule(s) dropped", grammar.dropped)
                }
                (_, Status::Ok, _) => "ok".to_string(),
                (_, Status::Slow, _) => "slow grammar".to_string(),
                (_, Status::Rejected, _) => grammar
                    .error
                    .clone()
                    .map(|e| format!("rejected: {e}"))
                    .unwrap_or_else(|| "rejected".to_string()),
            };
            let colour = match (grammar.status, &grammar.resolution) {
                (Status::Rejected, _) | (_, Resolution::Failed { .. }) => t.color_deleted,
                (Status::Slow, _) | (_, Resolution::NeedsBuild) => t.color_modified,
                _ => t.text,
            };
            panel = panel.child(
                div()
                    .text_size(FONT_SIZE_SM())
                    .child(format!("{} ({kind})", grammar.name))
                    .child(
                        div()
                            .text_color(colour)
                            .text_size(FONT_SIZE_SM())
                            .min_w_0()
                            .whitespace_normal()
                            .child(status),
                    ),
            );
        }
        panel = panel.child(
            div().mt(SPACING()).child(
                link_button("lang-ext-reveal", crate::context_menu::labels::REVEAL_IN_FILE_MANAGER, cx)
                    .text_size(FONT_SIZE_SM())
                    .on_click(move |_, _, cx| Dispatcher::show_in_finder(&dir, cx)),
            ),
        );
        div()
            .id("lang-ext-details-scroll")
            .size_full()
            .overflow_y_scroll()
            .child(panel)
            .with_scrollbar()
            .into_any_element()
    }
}

/// The row's dot and state text.
fn extension_state(installed: &Installed, extensions: &ExtensionsState) -> (Hsla, String) {
    let md = &installed.metadata;
    let (dot, text) = if extensions.progress.contains_key(&md.id) {
        (hsla(0.12, 0.8, 0.5, 1.), "Updating…".to_string())
    } else if !md.enabled {
        (hsla(0., 0., 0.6, 1.), "Disabled".to_string())
    } else if !md.usable() {
        (hsla(0., 0.7, 0.5, 1.), "No usable grammar".to_string())
    } else if md.grammars.iter().any(|g| {
        g.status == Status::Rejected || matches!(g.resolution, Resolution::NeedsBuild | Resolution::Failed { .. })
    }) {
        (hsla(0.12, 0.8, 0.5, 1.), "Partly working".to_string())
    } else if extensions.rebuilding {
        (hsla(0.33, 0.6, 0.45, 1.), "Loading…".to_string())
    } else {
        (hsla(0.33, 0.6, 0.45, 1.), "Enabled".to_string())
    };
    (dot, text)
}

fn grammar_kind_label(installed: &Installed) -> &'static str {
    let kinds: Vec<GrammarKind> = installed.metadata.grammars.iter().map(|g| g.kind).collect();
    let tree_sitter = kinds.contains(&GrammarKind::TreeSitter);
    let textmate = kinds.iter().any(|k| matches!(k, GrammarKind::TextMate | GrammarKind::Sublime));
    match (textmate, tree_sitter) {
        (true, true) => "TextMate + tree-sitter",
        (false, true) => "tree-sitter",
        _ => "TextMate",
    }
}

impl Render for LanguageExtensionsDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let extensions = self.state.read(cx).extensions.clone();
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_language_extensions(cx);
        let visible = self.visible(&extensions, cx);
        let mut pending: Vec<(&String, &ExtensionProgress)> = extensions
            .progress
            .iter()
            .filter(|(k, _)| k.starts_with("pending:"))
            .collect();
        pending.sort_by(|a, b| a.0.cmp(b.0));
        let pending_errors: Vec<String> = extensions
            .errors
            .iter()
            .filter(|(k, _)| k.starts_with("pending:"))
            .map(|(k, e)| {
                let what = k
                    .strip_prefix("pending:")
                    .and_then(|p| std::path::Path::new(p).file_name())
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                format!("{what}: {e}")
            })
            .collect();
        let count = extensions.installed.len();
        let summary = match count {
            0 => "No extensions installed.".to_string(),
            1 => "1 extension installed.".to_string(),
            n => format!("{n} extensions installed."),
        };
        let list_body: AnyElement = if visible.is_empty() && pending.is_empty() {
            div()
                .p(SPACING())
                .text_size(FONT_SIZE_SM())
                .text_color(t.text_secondary)
                .child(if count == 0 {
                    "Nothing installed yet. Use Add File or Folder… below."
                } else {
                    "No extension matches the filter."
                })
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .children(pending.iter().map(|(k, p)| self.pending_row(k, p, cx)))
                .children(visible.iter().map(|i| self.row(i, &extensions, cx)))
                .into_any_element()
        };
        let list = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(t.box_border)
            .child(
                div()
                    .p(SPACING())
                    .border_b_1()
                    .border_color(t.box_border)
                    .child(filter_text_box(
                        "lang-ext-filter",
                        &self.filter,
                        Some(octicon(Octicon::Search, t.text_secondary)),
                        window,
                        cx,
                    )),
            )
            .child(
                div()
                    .id("lang-ext-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(list_body)
                    .with_scrollbar(),
            )
            .children(pending_errors.into_iter().map(|e| {
                div()
                    .px(SPACING())
                    .py(SPACING_HALF())
                    .text_size(FONT_SIZE_SM())
                    .text_color(t.color_deleted)
                    .border_t_1()
                    .border_color(t.box_border)
                    .child(e)
            }));
        let details = div()
            .w(zpx(DETAILS_WIDTH))
            .flex_none()
            .bg(t.box_alt_background)
            .child(self.details(&extensions, cx));
        let content = div()
            .w(crate::theme::fit_width(WIDTH))
            .h(zpx(HEIGHT))
            .flex()
            .flex_row()
            .child(list)
            .child(details);
        let footer = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(SPACING())
            .px(SPACING_DOUBLE())
            .py(SPACING())
            .border_t_1()
            .border_color(t.box_border)
            .child(
                button("lang-ext-add", "Add File or Folder…", cx)
                    .on_click(cx.listener(|this, _, window, cx| this.choose(window, cx))),
            )
            .child(settings_description(cx).mt(zpx(0.)).child(summary))
            .child(div().flex_1())
            .child(
                primary_button("lang-ext-close", "Close", false, cx)
                    .on_click(|_, _, cx| Dispatcher::close_language_extensions(cx)),
            );
        dialog_with_frame(
            "language-extensions",
            "Language Extensions",
            content,
            Vec::new(),
            DialogFrame {
                content_padding: false,
                footer: Some(footer.into_any_element()),
                ..Default::default()
            },
            close,
            window,
            cx,
        )
    }
}

#[allow(dead_code)]
fn _unused(_: PathBuf) {}
