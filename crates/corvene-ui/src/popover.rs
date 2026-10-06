//! GHD `Popover` with `PopoverDecoration.Balloon` (`app/src/ui/lib/popover.tsx`,
//! `app/styles/ui/_popover.scss`): a `.popover-component` box placed under
//! its anchor by floating-ui (`computePosition` with `offset(TipSize)` and
//! `shift({ padding: 10 })`) and a tip pointing at the anchor's centre.
//!
//! Only the bottom placements and `RightBottom` (the commit form avatar's)
//! are ported (the ones the balloons use here); floating-ui's `flip` never
//! applies to them in the main window.

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
    balloon_popover_zoomed(anchor, position, 1., component, cx)
}

/// [`balloon_popover`] inside a page CSS-`zoom`ed by `zoom`
/// (`#no-repositories`): floating-ui places the popover from the anchor's
/// page rect, but its `top` / `left` and the tip apply inside the zoom, so
/// the place (the tip's offset included) and the tip scale by `zoom` while
/// the anchor's rect does not.
pub fn balloon_popover_zoomed(
    anchor: Bounds<Pixels>,
    position: PopoverAnchorPosition,
    zoom: f32,
    component: impl IntoElement,
    cx: &App,
) -> Deferred {
    let t = cx.ghd();
    let page_top = crate::theme::page_top();
    let (corner, x) = match position {
        PopoverAnchorPosition::BottomLeft => (Anchor::TopLeft, anchor.left()),
        PopoverAnchorPosition::BottomRight => (Anchor::TopRight, anchor.right()),
    };
    let y = (anchor.bottom() - page_top + zpx(TIP_SIZE)) * zoom + page_top;
    let anchor_center = anchor.center().x;
    deferred(
        anchored()
            .anchor(corner)
            .position(point(x * zoom, y))
            .shift_horizontally(zpx(SCREEN_BORDER_PADDING))
            .child(div().relative().child(component).child(tip(
                anchor_center,
                zoom,
                t.box_border,
                t.background,
            ))),
    )
}

/// GHD `PopoverAnchorPosition.RightBottom` (`right-end`): `component` to
/// the right of `anchor` with their bottom edges aligned and the tip on its
/// left edge pointing at the anchor's centre.
pub fn balloon_popover_right_bottom(
    anchor: Bounds<Pixels>,
    component: impl IntoElement,
    cx: &App,
) -> Deferred {
    let t = cx.ghd();
    let anchor_center = anchor.center().y;
    // floating-ui keeps the arrow `TipCornerPadding` inside the bottom
    // border, which can push the popover below the anchor's bottom edge
    let bottom = anchor
        .bottom()
        .max(anchor_center + zpx(TIP_SIZE + TIP_CORNER_PADDING + 1.));
    deferred(
        anchored()
            .anchor(Anchor::BottomLeft)
            .position(point(anchor.right() + zpx(TIP_SIZE), bottom))
            .child(div().relative().child(component).child(side_tip(
                anchor_center,
                t.box_border,
                t.background,
            ))),
    )
}

/// [`tip`] on the popover's left edge, pointing left at `anchor_center`.
fn side_tip(anchor_center: Pixels, border: Hsla, background: Hsla) -> AnyElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let unit = zpx(1.);
            let page = |v: Pixels| f32::from(v) / f32::from(unit);
            let height = page(bounds.size.height);
            let tip_box = TIP_SIZE * 2.;
            let arrow_y = (page(anchor_center) - page(bounds.top()) - TIP_SIZE)
                .min(height - tip_box - TIP_CORNER_PADDING)
                .max(TIP_CORNER_PADDING);
            let at = |x: f32, y: f32| {
                point(bounds.left() + zpx(x), bounds.top() + zpx(1. + arrow_y + y))
            };
            let tri = |apex: f32, base: f32| {
                let mut path = Path::new(at(apex, TIP_SIZE));
                path.line_to(at(base, tip_box));
                path.line_to(at(base, 0.));
                path.line_to(at(apex, TIP_SIZE));
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

/// `.popover-tip` above the popover: floating-ui's `arrow` puts its 16 px
/// box on the anchor's centre, at least `TipCornerPadding` from either end,
/// and GHD applies that offset inside the 1 px border (all in page px,
/// scaled by `zoom` on screen). Within it, rotated to point up, the
/// `--box-border-color` triangle sits a pixel above the background one,
/// which covers the popover's top border.
fn tip(anchor_center: Pixels, zoom: f32, border: Hsla, background: Hsla) -> AnyElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let unit = zpx(1.);
            let page = |v: Pixels| f32::from(v) / f32::from(unit);
            let width = page(bounds.size.width) / zoom;
            let tip_box = TIP_SIZE * 2.;
            let arrow_x = (page(anchor_center) - page(bounds.left()) / zoom - TIP_SIZE)
                .min(width - tip_box - TIP_CORNER_PADDING)
                .max(TIP_CORNER_PADDING);
            let at = |x: f32, y: f32| {
                point(
                    bounds.left() + zpx((1. + arrow_x + x) * zoom),
                    bounds.top() + zpx(y * zoom),
                )
            };
            let tri = |apex: f32, base: f32| {
                let mut path = Path::new(at(TIP_SIZE, apex));
                path.line_to(at(tip_box, base));
                path.line_to(at(0., base));
                path.line_to(at(TIP_SIZE, apex));
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
