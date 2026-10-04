//! `#cloning-repository-view`: shown in the content area while `git clone` runs
//! (GHD `app/src/ui/cloning-repository.tsx`).
//!
//! Deviation (flag `234-clone-cancel`): a Cancel button stops the clone; GHD
//! has none.
//!
//! Deviation (flag `293-clone-multiple`): a clone of several says "Cloning 2
//! of 5" under the title.

use corvene_core::{CloneState, Dispatcher};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::icons::{Octicon, octicon};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;

pub fn cloning_view(clone: &CloneState, cancellable: bool, cx: &App) -> impl IntoElement {
    let t = cx.ghd();
    let fraction = clone.value.unwrap_or(0.0).clamp(0.0, 1.0);
    div()
        .id("cloning-repository-view")
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(t.background)
        .child(
            div()
                .w(crate::theme::fit_width(600.))
                .mt(zpx(-60.))
                .p(SPACING_DOUBLE())
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .mb(SPACING())
                        .child(
                            octicon(Octicon::DesktopDownload, t.text)
                                .size(zpx(32.))
                                .mr(SPACING())
                                .mt(zpx(4.)),
                        )
                        .child(
                            div()
                                .text_size(zpx(32.))
                                .font_weight(FontWeight::LIGHT)
                                .truncate()
                                // GHD `Cloning {repository.name}`
                                .child(format!("Cloning {}", clone.name())),
                        ),
                )
                // Corvene (`293-clone-multiple`): the clone's place in the queue
                .when_some(clone.queue, |d, (ix, total)| {
                    d.child(
                        div()
                            .mb(SPACING())
                            .text_size(FONT_SIZE())
                            .text_color(t.text_secondary)
                            .child(format!("Cloning {ix} of {total}")),
                    )
                })
                .child(
                    // `progress`: 10 px track, text-colour fill
                    div()
                        .w_full()
                        .h(zpx(10.))
                        .rounded(zpx(5.))
                        .bg(t.box_alt_background)
                        .overflow_hidden()
                        .child(div().h_full().w(relative(fraction)).bg(t.text)),
                )
                .child(
                    div()
                        .mt(SPACING())
                        .text_size(FONT_SIZE())
                        .text_color(t.text_secondary)
                        .truncate()
                        .child(clone.description.clone()),
                )
                .when(cancellable, |d| {
                    let cancelling = clone.cancel.is_cancelled();
                    d.child(
                        div().mt(SPACING_DOUBLE()).flex().justify_end().child(
                            crate::widgets::button("cancel-clone", "Cancel", cx)
                                .when(cancelling, |b| b.opacity(0.6))
                                .when(!cancelling, |b| {
                                    b.on_click(|_, _, cx| Dispatcher::cancel_clone(cx))
                                }),
                        ),
                    )
                }),
        )
}
