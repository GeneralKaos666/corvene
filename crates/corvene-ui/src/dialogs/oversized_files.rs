//! GHD `OversizedFiles` (`app/src/ui/changes/oversized-files-warning.tsx`,
//! `styles/ui/changes/_oversized-files-warning.scss`): included files over
//! 100 MiB that Git LFS does not track; "Commit Anyway" commits them
//! (`corvene_core::commit_checks`).
//!
//! Deviation: with `784-suggest-lfs-tracking` (Git LFS installed) a third
//! button tracks the files' extensions in Git LFS and goes back to the
//! commit form.

use corvene_core::Dispatcher;
use corvene_core::commit_checks::CommitChecks;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::mac_or;
use crate::dialog::{
    DialogButton, DialogKind, GroupButtonSpec, OkCancelButtonGroup, dialog_with_kind,
};
use crate::scrollbar::ScrollbarExt;
use crate::theme::ActiveGhdTheme;
use crate::theme::mono_font;
use crate::theme::sizes::*;

/// GHD `GitLFSWebsiteURL`.
const GIT_LFS_WEBSITE_URL: &str = "https://help.github.com/articles/versioning-large-files/";

pub struct OversizedFilesDialog {
    repo: u64,
    files: Vec<String>,
    summary: String,
    description: String,
    lfs_patterns: Vec<String>,
    checks: CommitChecks,
}

impl OversizedFilesDialog {
    pub fn new(
        repo: u64,
        files: Vec<String>,
        summary: String,
        description: String,
        lfs_patterns: Vec<String>,
        checks: CommitChecks,
    ) -> Self {
        Self {
            repo,
            files,
            summary,
            description,
            lfs_patterns,
            checks,
        }
    }
}

impl Render for OversizedFilesDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd().clone();
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let content = div()
            .flex()
            .flex_col()
            // `.dialog-content { max-height: 305px }`
            .max_h(zpx(305.))
            .child({
                // `<p>` with a `<strong>` sentence
                let lead = "The following files are over 100MB. ";
                let strong = "If you commit these files, you will no longer be able to push \
                              this repository to GitHub.com.";
                div().mb(SPACING()).child(
                    StyledText::new(format!("{lead}{strong}")).with_highlights([(
                        lead.len()..lead.len() + strong.len(),
                        HighlightStyle {
                            font_weight: Some(FontWeight::BOLD),
                            ..Default::default()
                        },
                    )]),
                )
            })
            .child(
                // `.files-list`: monospace, 175 px, scrolls
                div()
                    .id("oversized-files-list")
                    .max_h(zpx(175.))
                    .overflow_y_scroll()
                    .mb(SPACING())
                    .flex()
                    .flex_col()
                    .font_family(mono_font())
                    .children(self.files.iter().map(|p| {
                        // `PathText`: the directory in the secondary colour
                        let (dir, name) = match p.rfind('/') {
                            Some(i) => (&p[..=i], &p[i + 1..]),
                            None => ("", p.as_str()),
                        };
                        let dir = crate::format::display_path(dir);
                        div()
                            .flex()
                            .flex_row()
                            .line_height(zpx(18.))
                            .when(!dir.is_empty(), |d| {
                                d.child(div().text_color(t.text_secondary).child(dir.to_string()))
                            })
                            .child(name.to_string())
                    }))
                    .with_scrollbar(),
            )
            .child(crate::widgets::paragraph(vec![
                "We recommend you avoid committing these files or use ".into(),
                crate::widgets::link_button("oversized-files-lfs-link", "Git LFS", cx)
                    .on_click(|_, _, cx| Dispatcher::open_url(GIT_LFS_WEBSITE_URL, cx))
                    .into_any_element()
                    .into(),
                " to store large files on GitHub.".into(),
            ]))
            // `784-suggest-lfs-tracking`
            .when(!self.lfs_patterns.is_empty(), |d| {
                d.child(div().mt(SPACING()).text_color(t.text_secondary).child(
                    "Tracking them in Git LFS adds the patterns to .gitattributes, which \
                             goes into the commit with the files.",
                ))
            });
        let (repo, summary, description, checks) = (
            self.repo,
            self.summary.clone(),
            self.description.clone(),
            self.checks.clone(),
        );
        let buttons = OkCancelButtonGroup {
            destructive: true,
            cancel: GroupButtonSpec {
                id: "oversized-files-cancel",
                label: "Cancel".into(),
                disabled: false,
                on_click: Box::new(close),
            },
            ok: GroupButtonSpec {
                id: "oversized-files-ok",
                label: mac_or("Commit Anyway", "Commit anyway").into(),
                disabled: false,
                on_click: Box::new(move |_, cx| {
                    Dispatcher::close_popup(cx);
                    let checks = CommitChecks {
                        allow_oversized: true,
                        ..checks.clone()
                    };
                    Dispatcher::commit_with(repo, summary.clone(), description.clone(), checks, cx);
                }),
            },
        }
        .into_buttons();
        // `784-suggest-lfs-tracking`: first in the macOS order (leftmost)
        let mut buttons = buttons;
        if !self.lfs_patterns.is_empty() {
            let label: SharedString = if self.lfs_patterns.len() <= 2 {
                format!("Track {} in Git LFS", self.lfs_patterns.join(", ")).into()
            } else {
                mac_or(
                    "Track These Files in Git LFS",
                    "Track these files in Git LFS",
                )
                .into()
            };
            let (patterns, files) = (self.lfs_patterns.clone(), self.files.clone());
            buttons.insert(
                0,
                DialogButton {
                    id: "oversized-files-track-lfs",
                    label,
                    primary: false,
                    disabled: false,
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::close_popup(cx);
                        Dispatcher::track_in_lfs(repo, patterns.clone(), files.clone(), cx);
                    }),
                },
            );
        }
        dialog_with_kind(
            "oversized-files",
            DialogKind::Warning,
            mac_or("Files Too Large", "Files too large"),
            content,
            buttons,
            close,
            window,
            cx,
        )
    }
}
