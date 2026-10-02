//! GHD `Popover` with `PopoverDecoration.Balloon` (`app/src/ui/lib/popover.tsx`,
//! `app/styles/ui/_popover.scss`): a `.popover-component` box placed under
//! its anchor by floating-ui (`computePosition` with `offset(TipSize)` and
//! `shift({ padding: 10 })`) and a tip pointing at the anchor's centre.
//!
//! Only the bottom placements are ported (the ones the balloons use here);
//! floating-ui's `flip` never applies to them in the main window.

use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;

/// `TipSize`: the tip's half width, and the gap it leaves to the anchor.
const TIP_SIZE: f32 = 8.;
/// `TipCornerPadding`: the tip stays this far from the popover's corners.
const TIP_CORNER_PADDING: f32 = TIP_SIZE;
/// `PopoverScreenBorderPadding`.
const SCREEN_BORDER_PADDING: f32 = 10.;

/// GHD `PopoverAnchorPosition` (the bottom ones).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopoverAnchorPosition {
    /// `bottom-start`: left edges aligned.
    BottomLeft,
    /// `bottom-end`: right edges aligned.
    BottomRight,
}

/// `.popover-component`: background, base border, border radius and
/// `--base-box-shadow` (`0 2px 7px var(--shadow-color)`). Callers add the
/// width and `.popover-content`'s padding.
pub fn popover_component(cx: &App) -> Div {
    let t = cx.ghd();
    div()
        .flex()
        .flex_col()
        .bg(t.background)
        .text_color(t.text)
        .border_1()
        .border_color(t.box_border)
        .rounded(BORDER_RADIUS())
        .shadow(vec![BoxShadow {
            color: t.shadow,
            offset: point(zpx(0.), zpx(2.)),
            blur_radius: css_blur(7.),
            spread_radius: zpx(0.),
            inset: false,
        }])
}

/// `component` (a [`popover_component`]) as a balloon under `anchor`
/// (window coordinates), drawn above everything else.
pub fn balloon_popover(
    anchor: Bounds<Pixels>,
    position: PopoverAnchorPosition,
    component: impl IntoElement,
    cx: &App,
) -> Deferred {
    let t = cx.ghd();
    let (corner, x) = match position {
        PopoverAnchorPosition::BottomLeft => (Anchor::TopLeft, anchor.left()),
        PopoverAnchorPosition::BottomRight => (Anchor::TopRight, anchor.right()),
    };
    let anchor_center = anchor.center().x;
    deferred(
        anchored()
            .anchor(corner)
            .position(point(x, anchor.bottom() + zpx(TIP_SIZE)))
            .snap_to_window_with_margin(zpx(SCREEN_BORDER_PADDING))
            .child(div().relative().child(component).child(tip(
                anchor_center,
                t.box_border,
                t.background,
            ))),
    )
}

/// `.popover-tip` above the popover: floating-ui's `arrow` puts its 16 px
/// box on the anchor's centre, at least `TipCornerPadding` from either end,
/// and GHD applies that offset inside the 1 px border. Within it, rotated
/// to point up, the `--box-border-color` triangle sits a pixel above the
/// background one, which covers the popover's top border.
fn tip(anchor_center: Pixels, border: Hsla, background: Hsla) -> AnyElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let width = f32::from(bounds.size.width);
            let tip_box = TIP_SIZE * 2.;
            let arrow_x = (f32::from(anchor_center - bounds.left()) - TIP_SIZE)
                .min(width - tip_box - TIP_CORNER_PADDING)
                .max(TIP_CORNER_PADDING);
            let left = bounds.left() + zpx(1. + arrow_x);
            let top = bounds.top();
            let tri = |apex: f32, base: f32| {
                let mut path = Path::new(point(left + zpx(TIP_SIZE), top + zpx(apex)));
                path.line_to(point(left + zpx(tip_box), top + zpx(base)));
                path.line_to(point(left, top + zpx(base)));
                path.line_to(point(left + zpx(TIP_SIZE), top + zpx(apex)));
                path
            };
            window.paint_path(tri(-7., 0.), border);
            window.paint_path(tri(-6., 1.), background);
        },
    )
    .absolute()
    .inset_0()
    .into_any_element()
}
