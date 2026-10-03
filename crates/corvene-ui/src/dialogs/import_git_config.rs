//! "Import Git Settings": the confirmation for the `importGitConfig` link
//! (`corvene_core::git_config_import`), which Termux's `corvene-git-config`
//! command opens. Corvene on Android only; GHD has no equivalent.

use corvene_core::Dispatcher;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::dialog::{GroupButtonSpec, OkCancelButtonGroup, dialog};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;

pub struct ImportGitConfigDialog {
    settings: Vec<(String, String)>,
    skipped: usize,
}

impl ImportGitConfigDialog {
    pub fn new(settings: Vec<(String, String)>, skipped: usize) -> Self {
        Self { settings, skipped }
    }
}

impl Render for ImportGitConfigDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let close = |_: &mut Window, cx: &mut App| Dispatcher::close_popup(cx);
        let settings = self.settings.clone();
        let left_out = match self.skipped {
            0 => String::new(),
            1 => " One more was left out: it names a program or a file Corvene does not have."
                .to_string(),
            n => format!(
                " {n} more were left out: they name programs or files Corvene does not have."
            ),
        };
        let content = div()
            .w(crate::theme::fit_width(460.))
            .flex()
            .flex_col()
            .gap(SPACING())
            .child(if self.settings.is_empty() {
                format!("There is no Git setting Corvene can use.{left_out}")
            } else {
                format!(
                    "Another application offers these Git settings. Corvene's Git will use \
                     them where its own settings say nothing else.{left_out}"
                )
            })
            .when(!self.settings.is_empty(), |d| {
                d.child(
                    div()
                        .id("import-git-config-list")
                        .max_h(zpx(240.))
                        .overflow_y_scroll()
                        .p(SPACING_HALF())
                        .rounded(BORDER_RADIUS())
                        .border_1()
                        .border_color(t.box_border)
                        .font_family(crate::theme::mono_font())
                        .text_size(FONT_SIZE_SM())
                        .flex()
                        .flex_col()
                        .children(
                            self.settings
                                .iter()
                                .map(|(key, value)| div().child(format!("{key} = {value}"))),
                        ),
                )
            });
        dialog(
            "import-git-config",
            "Import Git settings",
            content,
            OkCancelButtonGroup {
                destructive: false,
                cancel: GroupButtonSpec {
                    id: "import-git-config-cancel",
                    label: "Cancel".into(),
                    disabled: false,
                    on_click: Box::new(|_, cx| Dispatcher::close_popup(cx)),
                },
                ok: GroupButtonSpec {
                    id: "import-git-config-ok",
                    label: "Import".into(),
                    disabled: self.settings.is_empty(),
                    on_click: Box::new(move |_, cx| {
                        Dispatcher::close_popup(cx);
                        Dispatcher::import_git_config(settings.clone(), cx);
                    }),
                },
            }
            .into_buttons(),
            close,
            window,
            cx,
        )
    }
}
